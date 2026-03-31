declare const chrome: any;

import type {
  NativeHostSiteMatch,
  NativeHostStatusResponse,
  PopupFillResultResponse,
  PopupToBackgroundMessage,
  PopupToBackgroundResponse,
} from "@termkey/types";

document.body.innerHTML = `
  <main style="font-family: sans-serif; min-width: 320px; padding: 16px;">
    <h1 style="font-size: 18px; margin: 0 0 8px;">TermKey</h1>
    <p style="margin: 0 0 12px;">Extension ID: <code>${chrome.runtime.id}</code></p>
    <label style="display: block; margin: 0 0 8px;">
      <span style="display: block; font-size: 12px; margin-bottom: 4px;">Master password</span>
      <input id="master-password" type="password" style="box-sizing: border-box; width: 100%; padding: 8px;" />
    </label>
    <div style="display: flex; gap: 8px;">
      <button id="refresh-status" style="padding: 8px 12px;">Refresh status</button>
      <button id="find-site-matches" style="padding: 8px 12px;">Find matches</button>
      <button id="fill-best-match" style="padding: 8px 12px;">Fill best match</button>
      <button id="unlock-vault" style="padding: 8px 12px;">Unlock</button>
      <button id="ping-native-host" style="padding: 8px 12px;">Ping host</button>
    </div>
    <pre id="native-host-status" style="background: #f4f4f5; border-radius: 8px; margin: 12px 0 0; padding: 12px; white-space: pre-wrap;">Ready to test the native bridge.</pre>
    <pre id="site-match-list" style="background: #f9fafb; border-radius: 8px; margin: 12px 0 0; padding: 12px; white-space: pre-wrap;">Current-site matches will appear here after unlock.</pre>
  </main>
`;

const statusEl = document.querySelector<HTMLPreElement>("#native-host-status");
const pingButton = document.querySelector<HTMLButtonElement>("#ping-native-host");
const refreshStatusButton =
  document.querySelector<HTMLButtonElement>("#refresh-status");
const findSiteMatchesButton =
  document.querySelector<HTMLButtonElement>("#find-site-matches");
const fillBestMatchButton =
  document.querySelector<HTMLButtonElement>("#fill-best-match");
const unlockButton = document.querySelector<HTMLButtonElement>("#unlock-vault");
const passwordInput =
  document.querySelector<HTMLInputElement>("#master-password");
const siteMatchListEl =
  document.querySelector<HTMLPreElement>("#site-match-list");

if (
  !statusEl ||
  !pingButton ||
  !refreshStatusButton ||
  !findSiteMatchesButton ||
  !fillBestMatchButton ||
  !unlockButton ||
  !passwordInput ||
  !siteMatchListEl
) {
  throw new Error("Popup UI failed to initialize.");
}

const statusOutput = statusEl;
const pingNativeHostButton = pingButton;
const refreshNativeStatusButton = refreshStatusButton;
const findCurrentSiteMatchesButton = findSiteMatchesButton;
const fillCurrentSiteBestMatchButton = fillBestMatchButton;
const unlockVaultButton = unlockButton;
const masterPasswordInput = passwordInput;
const siteMatchOutput = siteMatchListEl;
let currentSiteMatches: NativeHostSiteMatch[] = [];
let canFillBestMatch = false;

function renderStatus(message: string) {
  statusOutput.textContent = message;
}

function updateFillButtonState() {
  fillCurrentSiteBestMatchButton.disabled = !canFillBestMatch;
}

function renderSiteMatches(message: string) {
  siteMatchOutput.textContent = message;
}

function formatStatus(response: NativeHostStatusResponse) {
  return [
    "Native host status",
    "",
    `app: ${response.app}`,
    `version: ${response.version}`,
    `vaultPath: ${response.vaultPath}`,
    `vaultExists: ${response.vaultExists}`,
    `firstRunComplete: ${response.firstRunComplete}`,
    `recoveryConfigured: ${response.recoveryConfigured}`,
    `locked: ${response.locked}`,
  ].join("\n");
}

function sendMessage(
  message: PopupToBackgroundMessage,
  onSuccess: (response: PopupToBackgroundResponse) => void
) {
  chrome.runtime.sendMessage(
    message,
    (response: PopupToBackgroundResponse | undefined) => {
      const runtimeError = chrome.runtime.lastError;
      if (runtimeError) {
        renderStatus(`Background message failed:\n${runtimeError.message}`);
        return;
      }

      if (!response) {
        renderStatus("No response received from the extension background.");
        return;
      }

      onSuccess(response);
    }
  );
}

function formatSiteMatches(
  siteUrl: string,
  siteOrigin: string,
  siteHostname: string,
  matches: NativeHostSiteMatch[]
) {
  if (matches.length === 0) {
    return [
      "Current site",
      `${siteUrl}`,
      "",
      `origin: ${siteOrigin}`,
      `hostname: ${siteHostname}`,
      "",
      "No saved password entries match this site.",
    ].join("\n");
  }

  return [
    "Current site",
    `${siteUrl}`,
    "",
    `origin: ${siteOrigin}`,
    `hostname: ${siteHostname}`,
    "",
    "Matching entries",
    "",
    matches.map((entry, index) => {
      const lines = [
        `${index + 1}. ${entry.name}`,
        `   match: ${entry.matchType}`,
      ];

      if (entry.username) {
        lines.push(`   username: ${entry.username}`);
      }
      if (entry.url) {
        lines.push(`   url: ${entry.url}`);
      }

      return lines.join("\n");
    })
      .join("\n\n"),
  ].join("\n");
}

pingNativeHostButton.addEventListener("click", () => {
  renderStatus("Pinging native host...");

  const message: PopupToBackgroundMessage = { type: "termkey.nativeHost.ping" };
  sendMessage(message, (response) => {
      if (!response.ok) {
        renderStatus(`Native host unavailable:\n${response.error}`);
        return;
      }

      renderStatus(
        `Native host connected.\n\n${JSON.stringify(response.response, null, 2)}`
      );
    });
});

function refreshStatus() {
  renderStatus("Loading TermKey status...");

  const message: PopupToBackgroundMessage = { type: "termkey.nativeHost.status" };
  sendMessage(message, (response) => {
    if (!response.ok) {
      renderStatus(`Native host unavailable:\n${response.error}`);
      return;
    }

    if (response.response.type !== "status") {
      renderStatus("Native host returned the wrong response type for status.");
      return;
    }

    renderStatus(formatStatus(response.response));

    if (response.response.locked) {
      currentSiteMatches = [];
      canFillBestMatch = false;
      updateFillButtonState();
      renderSiteMatches("Unlock the vault to find matches for the current site.");
      return;
    }

    findSiteMatches();
  });
}

refreshNativeStatusButton.addEventListener("click", refreshStatus);

function findSiteMatches() {
  renderSiteMatches("Loading matches for the current site...");

  const message: PopupToBackgroundMessage = {
    type: "termkey.nativeHost.findSiteMatches",
  };
  sendMessage(message, (response) => {
    if (!response.ok) {
      currentSiteMatches = [];
      canFillBestMatch = false;
      updateFillButtonState();
      renderSiteMatches(`Find matches failed:\n${response.error}`);
      return;
    }

    if (response.response.type !== "site_matches") {
      currentSiteMatches = [];
      canFillBestMatch = false;
      updateFillButtonState();
      renderSiteMatches("Native host returned the wrong response type for site matches.");
      return;
    }

    renderSiteMatches(
      formatSiteMatches(
        response.response.siteUrl,
        response.response.siteOrigin,
        response.response.siteHostname,
        response.response.matches
      )
    );
    currentSiteMatches = response.response.matches;
    canFillBestMatch = currentSiteMatches.length > 0;
    updateFillButtonState();
  });
}

findCurrentSiteMatchesButton.addEventListener("click", findSiteMatches);

fillCurrentSiteBestMatchButton.addEventListener("click", () => {
  const bestMatch = currentSiteMatches[0];
  if (!bestMatch) {
    renderStatus("No current-site match is available to fill.");
    return;
  }

  renderStatus(`Filling ${bestMatch.name} into the current page...`);
  sendMessage(
    {
      type: "termkey.autofill.fillBestMatch",
      entryId: bestMatch.id,
    },
    (response) => {
      if (!response.ok) {
        renderStatus(`Autofill failed:\n${response.error}`);
        return;
      }

      if (response.response.type !== "fill_result") {
        renderStatus("Background returned the wrong response type for autofill.");
        return;
      }

      const result: PopupFillResultResponse = response.response;
      renderStatus(
        `Filled ${result.entryName} into the current page.\n\nFields updated: ${result.filledFields}`
      );
    }
  );
});

function unlockVault() {
  const password = masterPasswordInput.value;
  if (!password) {
    renderStatus("Enter your master password to unlock the vault.");
    return;
  }

  renderStatus("Unlocking vault...");

  const message: PopupToBackgroundMessage = {
    type: "termkey.nativeHost.unlock",
    password,
  };

  sendMessage(message, (response) => {
    masterPasswordInput.value = "";

    if (!response.ok) {
      renderStatus(`Unlock failed:\n${response.error}`);
      return;
    }

    if (response.response.type !== "unlock") {
      renderStatus("Native host returned the wrong response type for unlock.");
      return;
    }

    renderStatus("Vault unlocked for the current native host session.");
    refreshStatus();
  });
}

unlockVaultButton.addEventListener("click", unlockVault);
masterPasswordInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    unlockVault();
  }
});

updateFillButtonState();
refreshStatus();
