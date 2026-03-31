declare const chrome: any;

import { coreReady } from "@termkey/core";
import type {
  NativeHostRequest,
  NativeHostResponse,
  PopupToBackgroundMessage,
  PopupToBackgroundResponse,
} from "@termkey/types";

const NATIVE_HOST_NAME = "com.ryanonmars.termkey";
let nativePort: any | undefined;
let currentNativeResponseHandler:
  | ((response: PopupToBackgroundResponse) => void)
  | undefined;
let nativeRequestQueue: Promise<void> = Promise.resolve();

function isMissingContentScriptError(message: string | undefined) {
  if (!message) {
    return false;
  }

  return (
    message.includes("Receiving end does not exist") ||
    message.includes("Could not establish connection")
  );
}

function sendMessageToTab<TResponse>(
  tabId: number,
  message: unknown
): Promise<TResponse> {
  return new Promise<TResponse>((resolve, reject) => {
    chrome.tabs.sendMessage(tabId, message, (response: TResponse | undefined) => {
      const runtimeError = chrome.runtime.lastError;
      if (runtimeError) {
        reject(new Error(runtimeError.message));
        return;
      }

      resolve(response as TResponse);
    });
  });
}

async function ensureContentScript(tabId: number) {
  try {
    await sendMessageToTab(tabId, { type: "termkey.contentScriptProbe" });
    return;
  } catch (error) {
    if (
      !(error instanceof Error) ||
      !isMissingContentScriptError(error.message)
    ) {
      throw error;
    }
  }

  await chrome.scripting.executeScript({
    target: { tabId },
    files: ["dist/content.js"],
  });
}

async function getCurrentTabUrl(): Promise<string> {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const url = tab?.url;

  if (!url) {
    throw new Error("No active tab URL is available.");
  }

  if (!url.startsWith("http://") && !url.startsWith("https://")) {
    throw new Error("Current tab is not a supported web page.");
  }

  return url;
}

async function getCurrentTabId(): Promise<number> {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const tabId = tab?.id;

  if (typeof tabId !== "number") {
    throw new Error("No active tab is available for autofill.");
  }

  return tabId;
}

chrome.runtime.onInstalled.addListener(() => {
  console.log("TermKey extension installed");
});

chrome.runtime.onStartup.addListener(() => {
  console.log("TermKey extension started");
});

console.log("Core connected:", coreReady);
console.log("Extension ID:", chrome.runtime.id);

function handleNativeHostResponse(response: NativeHostResponse | undefined) {
  if (!currentNativeResponseHandler) {
    console.warn("Dropping unexpected native host response", response);
    return;
  }

  const resolve = currentNativeResponseHandler;
  currentNativeResponseHandler = undefined;

  if (!response) {
    resolve({ ok: false, error: "Native host returned an empty response." });
    return;
  }

  if (response.type === "error") {
    resolve({ ok: false, error: response.message });
    return;
  }

  if (
    response.type !== "pong" &&
    response.type !== "status" &&
    response.type !== "autofill_entry" &&
    response.type !== "site_matches" &&
    response.type !== "list_entries" &&
    response.type !== "unlock"
  ) {
    resolve({ ok: false, error: "Native host returned an invalid response." });
    return;
  }

  resolve({ ok: true, response });
}

function getNativePort() {
  if (nativePort) {
    return nativePort;
  }

  const port = chrome.runtime.connectNative(NATIVE_HOST_NAME);
  port.onMessage.addListener((message: NativeHostResponse | undefined) => {
    handleNativeHostResponse(message);
  });
  port.onDisconnect.addListener(() => {
    const runtimeError = chrome.runtime.lastError;
    nativePort = undefined;

    if (currentNativeResponseHandler) {
      const resolve = currentNativeResponseHandler;
      currentNativeResponseHandler = undefined;
      resolve({
        ok: false,
        error: runtimeError?.message ?? "Native host disconnected.",
      });
    }
  });

  nativePort = port;
  return port;
}

function enqueueNativeHostRequest(
  request: NativeHostRequest
): Promise<PopupToBackgroundResponse> {
  const result = nativeRequestQueue.then(
    () =>
      new Promise<PopupToBackgroundResponse>((resolve) => {
        const port = getNativePort();
        currentNativeResponseHandler = resolve;
        port.postMessage(request);
      })
  );

  nativeRequestQueue = result.then(
    () => undefined,
    () => undefined
  );

  return result;
}

chrome.runtime.onMessage.addListener(
  (
    message: PopupToBackgroundMessage,
    _sender: unknown,
    sendResponse: (response: PopupToBackgroundResponse) => void
  ) => {
    if (message?.type === "termkey.nativeHost.ping") {
      void enqueueNativeHostRequest({ type: "ping" }).then(sendResponse);
      return true;
    }

    if (message?.type === "termkey.nativeHost.status") {
      void enqueueNativeHostRequest({ type: "status" }).then(sendResponse);
      return true;
    }

    if (message?.type === "termkey.nativeHost.findSiteMatches") {
      void getCurrentTabUrl()
        .then((url) =>
          enqueueNativeHostRequest({ type: "find_site_matches", url })
        )
        .then(sendResponse)
        .catch((error: Error) => {
          sendResponse({ ok: false, error: error.message });
        });
      return true;
    }

    if (message?.type === "termkey.autofill.fillSelectedMatch") {
      void getCurrentTabId()
        .then((tabId) =>
          enqueueNativeHostRequest({
            type: "get_autofill_entry",
            id: message.entryId,
          }).then(
            (response): {
              response: PopupToBackgroundResponse;
              tabId: number;
            } => ({ response, tabId })
          )
        )
        .then(({ response, tabId }: { response: PopupToBackgroundResponse; tabId: number }) => {
          if (!response.ok) {
            sendResponse(response);
            return;
          }

          if (response.response.type !== "autofill_entry") {
            sendResponse({
              ok: false,
              error: "Native host returned the wrong response type for autofill.",
            });
            return;
          }

          const autofillEntry = response.response.entry;

          void ensureContentScript(tabId)
            .then(() =>
              sendMessageToTab<any>(tabId, {
                type: "termkey.fillCredentials",
                entry: autofillEntry,
              })
            )
            .then((fillResponse) => {
              if (!fillResponse?.ok) {
                sendResponse({
                  ok: false,
                  error:
                    fillResponse?.error ??
                    "Content script could not fill the page.",
                });
                return;
              }

              sendResponse({
                ok: true,
                response: {
                  type: "fill_result",
                  entryName: autofillEntry.name,
                  filledFields: fillResponse.filledFields ?? 0,
                  filledUsername: fillResponse.filledUsername === true,
                  filledPassword: fillResponse.filledPassword === true,
                },
              });
            })
            .catch((error: Error) => {
              sendResponse({ ok: false, error: error.message });
            });
        })
        .catch((error: Error) => {
          sendResponse({ ok: false, error: error.message });
        });
      return true;
    }

    if (message?.type === "termkey.nativeHost.unlock") {
      void enqueueNativeHostRequest({
        type: "unlock",
        password: message.password,
      }).then(sendResponse);
      return true;
    }

    return false;
  }
);
