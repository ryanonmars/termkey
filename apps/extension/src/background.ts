declare const chrome: any;

import { coreReady } from "@termkey/core";
import type {
  NativeHostRequest,
  NativeHostResponse,
  PopupPageIntent,
  PopupToBackgroundMessage,
  PopupToBackgroundResponse,
} from "@termkey/types";

const NATIVE_HOST_NAME = "com.ryanonmars.termkey";
const PENDING_SAVE_KEY_PREFIX = "pending-save:";
let nativePort: any | undefined;
let currentNativeResponseHandler:
  | ((response: PopupToBackgroundResponse) => void)
  | undefined;
let nativeRequestQueue: Promise<void> = Promise.resolve();

type PendingSaveDraft = {
  username: string;
  url: string;
};

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

function getPendingSaveKey(tabId: number) {
  return `${PENDING_SAVE_KEY_PREFIX}${tabId}`;
}

function getSessionStorageArea() {
  return chrome.storage?.session ?? chrome.storage.local;
}

function readPendingSaveDraft(tabId: number): Promise<PendingSaveDraft | null> {
  return new Promise((resolve) => {
    getSessionStorageArea().get([getPendingSaveKey(tabId)], (result: Record<string, unknown>) => {
      resolve((result?.[getPendingSaveKey(tabId)] as PendingSaveDraft | undefined) ?? null);
    });
  });
}

function writePendingSaveDraft(tabId: number, draft: PendingSaveDraft): Promise<void> {
  return new Promise((resolve) => {
    getSessionStorageArea().set({ [getPendingSaveKey(tabId)]: draft }, () => resolve());
  });
}

function clearPendingSaveDraft(tabId: number): Promise<void> {
  return new Promise((resolve) => {
    getSessionStorageArea().remove(getPendingSaveKey(tabId), () => resolve());
  });
}

function hostFromUrl(url: string) {
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

function canReusePendingSaveDraft(currentUrl: string, draft: PendingSaveDraft | null) {
  if (!draft) {
    return false;
  }

  const currentHost = hostFromUrl(currentUrl);
  const draftHost = hostFromUrl(draft.url);

  return Boolean(currentHost && draftHost && currentHost === draftHost);
}

chrome.tabs.onRemoved.addListener((tabId: number) => {
  void clearPendingSaveDraft(tabId);
});

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
    response.type !== "generated_password" &&
    response.type !== "save_entry" &&
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

    if (message?.type === "termkey.content.captureVisibleCredentials") {
      void Promise.all([getCurrentTabId(), getCurrentTabUrl()])
        .then(([tabId, url]) =>
          ensureContentScript(tabId).then(() =>
            sendMessageToTab<{
              ok: boolean;
              error?: string;
              captureState?: "complete" | "password_only" | "username_only";
              username?: string | null;
              password?: string;
            }>(tabId, {
              type: "termkey.captureVisibleCredentials",
            }).then((captureResponse) => ({ captureResponse, tabId, url }))
          )
        )
        .then(
          ({
            captureResponse,
            tabId,
            url,
          }: {
            captureResponse: {
              ok: boolean;
              error?: string;
              captureState?: "complete" | "password_only" | "username_only";
              username?: string | null;
              password?: string;
            };
            tabId: number;
            url: string;
          }) => {
            if (!captureResponse?.ok) {
              sendResponse({
                ok: false,
                error:
                  captureResponse?.error ??
                  "Could not read the current login fields from this page.",
              });
              return;
            }

            if (
              captureResponse.captureState === "username_only" &&
              captureResponse.username
            ) {
              void writePendingSaveDraft(tabId, {
                username: captureResponse.username,
                url,
              }).then(() => {
                sendResponse({
                  ok: true,
                  response: {
                    type: "captured_login_step",
                    step: "username_only",
                    username: captureResponse.username!,
                    url,
                  },
                });
              });
              return;
            }

            if (!captureResponse.password) {
              sendResponse({
                ok: false,
                error: "Could not read the current login password from this page.",
              });
              return;
            }

            void readPendingSaveDraft(tabId).then((draft) => {
              const useStoredUsername =
                !captureResponse.username && canReusePendingSaveDraft(url, draft);
              const mergedUsername = useStoredUsername
                ? draft?.username ?? null
                : captureResponse.username ?? null;

              void clearPendingSaveDraft(tabId).then(() => {
                sendResponse({
                  ok: true,
                  response: {
                    type: "captured_login",
                    candidate: {
                      username: mergedUsername,
                      password: captureResponse.password!,
                      url,
                    },
                    usedStoredUsername: useStoredUsername,
                  },
                });
              });
            });
          }
        )
        .catch((error: Error) => {
          sendResponse({ ok: false, error: error.message });
        });
      return true;
    }

    if (message?.type === "termkey.content.inspectPageContext") {
      void Promise.all([getCurrentTabId(), getCurrentTabUrl()])
        .then(([tabId, url]) =>
          ensureContentScript(tabId).then(() =>
            Promise.all([
              sendMessageToTab<{
                ok: boolean;
              error?: string;
              intent?: PopupPageIntent;
              visibleUsername?: string | null;
              hasPasswordField?: boolean;
              hasConfirmationPasswordField?: boolean;
              canGeneratePassword?: boolean;
            }>(tabId, {
              type: "termkey.inspectPageContext",
            }),
              readPendingSaveDraft(tabId),
            ]).then(([pageContext, draft]) => ({ pageContext, draft, url }))
          )
        )
        .then(
          ({
            pageContext,
            draft,
            url,
          }: {
            pageContext: {
              ok: boolean;
              error?: string;
              intent?: PopupPageIntent;
              visibleUsername?: string | null;
              hasPasswordField?: boolean;
              hasConfirmationPasswordField?: boolean;
              canGeneratePassword?: boolean;
            };
            draft: PendingSaveDraft | null;
            url: string;
          }) => {
            if (!pageContext?.ok) {
              sendResponse({
                ok: false,
                error:
                  pageContext?.error ??
                  "Could not inspect the current page.",
              });
              return;
            }

            const reusableDraft = canReusePendingSaveDraft(url, draft) ? draft : null;
            sendResponse({
              ok: true,
              response: {
                type: "page_context",
                context: {
                  intent: pageContext.intent ?? "unknown",
                  visibleUsername: pageContext.visibleUsername ?? null,
                  hasPasswordField: pageContext.hasPasswordField === true,
                  hasConfirmationPasswordField:
                    pageContext.hasConfirmationPasswordField === true,
                  canGeneratePassword: pageContext.canGeneratePassword === true,
                  hasPendingSaveUsername: reusableDraft !== null,
                  pendingUsername: reusableDraft?.username ?? null,
                },
              },
            });
          }
        )
        .catch((error: Error) => {
          sendResponse({ ok: false, error: error.message });
        });
      return true;
    }

    if (message?.type === "termkey.passwords.generateForPage") {
      void Promise.all([
        getCurrentTabId(),
        getCurrentTabUrl(),
        enqueueNativeHostRequest({ type: "generate_password" }),
      ])
        .then(([tabId, url, nativeResponse]) => {
          if (!nativeResponse.ok) {
            sendResponse(nativeResponse);
            return;
          }

          if (nativeResponse.response.type !== "generated_password") {
            sendResponse({
              ok: false,
              error: "Native host returned the wrong response type for password generation.",
            });
            return;
          }

          const generatedPassword = (
            nativeResponse.response as Extract<
              NativeHostResponse,
              { type: "generated_password" }
            >
          ).password;

          void ensureContentScript(tabId)
            .then(() =>
              sendMessageToTab<{
                ok: boolean;
                error?: string;
                username?: string | null;
                filledPasswordFields?: number;
              }>(tabId, {
                type: "termkey.fillGeneratedPassword",
                password: generatedPassword,
              })
            )
            .then((fillResponse) => {
              if (!fillResponse?.ok) {
                sendResponse({
                  ok: false,
                  error:
                    fillResponse?.error ??
                    "Content script could not fill generated password fields.",
                });
                return;
              }

              sendResponse({
                ok: true,
                response: {
                  type: "generated_password",
                  candidate: {
                    username: fillResponse.username ?? null,
                    password: generatedPassword,
                    url,
                  },
                  filledPasswordFields: fillResponse.filledPasswordFields ?? 0,
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

    if (message?.type === "termkey.autofill.fillSelectedMatch") {
      void getCurrentTabId()
        .then((tabId) =>
          enqueueNativeHostRequest({
            type: "get_autofill_entry",
            id: message.entryId,
            password: message.password,
            secondaryPassword: message.secondaryPassword,
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

    if (message?.type === "termkey.nativeHost.savePasswordEntry") {
      void enqueueNativeHostRequest({
        type: "save_password_entry",
        name: message.name,
        username: message.username,
        password: message.password,
        url: message.url,
        masterPassword: message.masterPassword,
        secondaryPassword: message.secondaryPassword,
      })
        .then((response) => {
          if (!response.ok) {
            sendResponse(response);
            return;
          }

          if (response.response.type !== "save_entry") {
            sendResponse({
              ok: false,
              error: "Native host returned the wrong response type for save.",
            });
            return;
          }

          sendResponse({
            ok: true,
            response: {
              type: "save_entry_result",
              entryName: response.response.entryName,
            },
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
