declare const chrome: any;

import type {
  NativeHostSiteMatch,
  PopupFillResultResponse,
  PopupToBackgroundMessage,
  PopupToBackgroundResponse,
} from "@termkey/types";

document.body.innerHTML = `
  <style>
    :root {
      color-scheme: dark;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
    }

    * {
      box-sizing: border-box;
    }

    body {
      margin: 0;
      min-width: 360px;
      background:
        radial-gradient(circle at top, rgba(34, 197, 94, 0.18), transparent 42%),
        linear-gradient(180deg, #121923 0%, #0b0f14 100%);
      color: #f3f4f6;
    }

    button,
    input {
      font: inherit;
    }

    .popup {
      display: grid;
      gap: 14px;
      padding: 18px;
    }

    .header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 12px;
    }

    .eyebrow {
      margin: 0 0 4px;
      color: #7dd3fc;
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .title {
      margin: 0;
      font-size: 22px;
      font-weight: 600;
      letter-spacing: -0.03em;
    }

    .subtitle {
      margin: 6px 0 0;
      color: #93a4b8;
      font-size: 13px;
      line-height: 1.4;
    }

    .connection {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      margin-top: 4px;
      padding: 8px 10px;
      border: 1px solid rgba(148, 163, 184, 0.14);
      border-radius: 999px;
      background: rgba(15, 23, 42, 0.72);
      color: #cbd5e1;
      font-size: 12px;
      white-space: nowrap;
    }

    .status-dot {
      width: 10px;
      height: 10px;
      border-radius: 999px;
      background: #ef4444;
      box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.12);
      transition: background 140ms ease, box-shadow 140ms ease;
    }

    .status-dot--online {
      background: #22c55e;
      box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.18);
    }

    .panel {
      padding: 14px;
      border: 1px solid rgba(148, 163, 184, 0.14);
      border-radius: 16px;
      background: rgba(15, 23, 42, 0.74);
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
    }

    .panel[hidden] {
      display: none;
    }

    .site-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
    }

    .site-label {
      display: block;
      margin-bottom: 6px;
      color: #7dd3fc;
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .site-hostname {
      display: block;
      font-size: 18px;
      font-weight: 600;
      line-height: 1.2;
      word-break: break-word;
    }

    .site-summary {
      display: block;
      margin-top: 5px;
      color: #93a4b8;
      font-size: 13px;
      line-height: 1.4;
    }

    .fill-button,
    .unlock-button {
      border: 0;
      border-radius: 12px;
      background: linear-gradient(180deg, #22c55e 0%, #16a34a 100%);
      color: #03120a;
      font-weight: 700;
      cursor: pointer;
      transition: transform 140ms ease, opacity 140ms ease;
    }

    .fill-button {
      min-width: 76px;
      padding: 10px 14px;
    }

    .unlock-button {
      padding: 11px 14px;
    }

    .fill-button:hover:not(:disabled),
    .unlock-button:hover:not(:disabled) {
      transform: translateY(-1px);
    }

    .fill-button:disabled,
    .unlock-button:disabled {
      opacity: 0.45;
      cursor: not-allowed;
      transform: none;
    }

    .unlock-label {
      display: block;
      color: #cbd5e1;
      font-size: 12px;
      font-weight: 600;
      margin-bottom: 8px;
    }

    .unlock-row {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px;
    }

    .password-input {
      width: 100%;
      padding: 11px 12px;
      border: 1px solid rgba(148, 163, 184, 0.18);
      border-radius: 12px;
      background: rgba(2, 6, 23, 0.72);
      color: #f8fafc;
      outline: none;
    }

    .password-input::placeholder {
      color: #64748b;
    }

    .password-input:focus {
      border-color: rgba(34, 197, 94, 0.6);
      box-shadow: 0 0 0 3px rgba(34, 197, 94, 0.14);
    }

    .hint,
    .message {
      margin: 0;
      font-size: 13px;
      line-height: 1.5;
    }

    .hint {
      margin-top: 9px;
      color: #93a4b8;
    }

    .message {
      padding: 0 2px;
      color: #cbd5e1;
    }

    .message[data-tone="success"] {
      color: #86efac;
    }

    .message[data-tone="error"] {
      color: #fca5a5;
    }
  </style>
  <main class="popup">
    <header class="header">
      <div>
        <p class="eyebrow">TermKey</p>
        <h1 class="title">Current site</h1>
        <p class="subtitle">Autofill is checked automatically when the popup opens.</p>
      </div>
      <div class="connection">
        <span id="backend-dot" class="status-dot" aria-hidden="true"></span>
        <span id="backend-label">Checking backend</span>
      </div>
    </header>

    <section id="site-panel" class="panel">
      <div class="site-row">
        <div>
          <span class="site-label">Detected site</span>
          <strong id="site-hostname" class="site-hostname">Waiting for page...</strong>
          <span id="site-summary" class="site-summary">Checking the current tab.</span>
        </div>
        <button id="fill-best-match" class="fill-button" disabled>Fill</button>
      </div>
    </section>

    <section id="unlock-section" class="panel">
      <label>
        <span class="unlock-label">Master password</span>
        <div class="unlock-row">
          <input
            id="master-password"
            class="password-input"
            type="password"
            placeholder="Unlock your vault"
            autocomplete="current-password"
          />
          <button id="unlock-vault" class="unlock-button">Unlock</button>
        </div>
      </label>
      <p class="hint">The popup will check this site again as soon as the vault unlocks.</p>
    </section>

    <p id="native-host-status" class="message">Checking TermKey status...</p>
  </main>
`;

const backendDotEl = document.querySelector<HTMLSpanElement>("#backend-dot");
const backendLabelEl = document.querySelector<HTMLSpanElement>("#backend-label");
const sitePanelEl = document.querySelector<HTMLElement>("#site-panel");
const fillBestMatchButton =
  document.querySelector<HTMLButtonElement>("#fill-best-match");
const unlockSectionEl =
  document.querySelector<HTMLElement>("#unlock-section");
const unlockButton = document.querySelector<HTMLButtonElement>("#unlock-vault");
const passwordInput =
  document.querySelector<HTMLInputElement>("#master-password");
const statusEl = document.querySelector<HTMLParagraphElement>("#native-host-status");
const siteHostnameEl =
  document.querySelector<HTMLSpanElement>("#site-hostname");
const siteSummaryEl =
  document.querySelector<HTMLSpanElement>("#site-summary");

if (
  !backendDotEl ||
  !backendLabelEl ||
  !sitePanelEl ||
  !fillBestMatchButton ||
  !unlockSectionEl ||
  !unlockButton ||
  !passwordInput ||
  !statusEl ||
  !siteHostnameEl ||
  !siteSummaryEl
) {
  throw new Error("Popup UI failed to initialize.");
}

type MessageTone = "neutral" | "success" | "error";

const backendDot = backendDotEl;
const backendLabel = backendLabelEl;
const sitePanel = sitePanelEl;
const fillButton = fillBestMatchButton;
const unlockSection = unlockSectionEl;
const unlockVaultButton = unlockButton;
const masterPasswordInput = passwordInput;
const statusMessage = statusEl;
const siteHostname = siteHostnameEl;
const siteSummary = siteSummaryEl;

let currentSiteMatches: NativeHostSiteMatch[] = [];
let backendConnected = false;
let vaultLocked = true;
let hasSupportedPage = true;
let siteDetails = {
  hostname: "Waiting for page...",
  summary: "Checking the current tab.",
};

function setSiteVisibility(visible: boolean) {
  hasSupportedPage = visible;
  sitePanel.hidden = !visible;
  if (!visible) {
    currentSiteMatches = [];
  }
  updateFillButtonState();
}

function primeCurrentSite() {
  chrome.tabs.query(
    { active: true, currentWindow: true },
    (tabs: Array<{ url?: string }>) => {
      const runtimeError = chrome.runtime.lastError;
      if (runtimeError) {
        setSiteVisibility(false);
        return;
      }

      const url = tabs[0]?.url;
      if (!url) {
        setSiteVisibility(false);
        return;
      }

      try {
        const parsedUrl = new URL(url);
        if (
          parsedUrl.protocol !== "http:" &&
          parsedUrl.protocol !== "https:"
        ) {
          setSiteVisibility(false);
          return;
        }

        setSiteVisibility(true);
        renderSite(parsedUrl.hostname, "Checking for a saved login...");
      } catch {
        setSiteVisibility(false);
      }
    }
  );
}

function setBackendStatus(connected: boolean, label: string) {
  backendConnected = connected;
  backendDot.classList.toggle("status-dot--online", connected);
  backendLabel.textContent = label;
  updateFillButtonState();
}

function renderMessage(message: string, tone: MessageTone = "neutral") {
  statusMessage.dataset.tone = tone;
  statusMessage.textContent = message;
}

function renderSite(hostnameText: string, summaryText: string) {
  siteDetails = {
    hostname: hostnameText,
    summary: summaryText,
  };
  siteHostname.textContent = siteDetails.hostname;
  siteSummary.textContent = siteDetails.summary;
}

function updateFillButtonState() {
  fillButton.disabled =
    !hasSupportedPage ||
    !backendConnected ||
    vaultLocked ||
    currentSiteMatches.length === 0;
}

function resetMatches(summaryText: string) {
  currentSiteMatches = [];
  renderSite(siteDetails.hostname, summaryText);
  updateFillButtonState();
}

function showUnlockSection(locked: boolean) {
  vaultLocked = locked;
  unlockSection.hidden = !locked;
  updateFillButtonState();
}

function describeMatches(matches: NativeHostSiteMatch[]) {
  if (matches.length === 0) {
    return "No saved login found for this site.";
  }

  const bestMatch = matches[0];
  const details = [bestMatch.name];

  if (bestMatch.username) {
    details.push(bestMatch.username);
  }

  const suffix = matches.length > 1 ? ` • ${matches.length} matches` : "";
  return `Best match: ${details.join(" • ")}${suffix}`;
}

function formatFillResultMessage(result: PopupFillResultResponse) {
  if (result.filledUsername && result.filledPassword) {
    return `Filled ${result.entryName}. Username and password updated.`;
  }

  if (result.filledUsername) {
    return `Filled username for ${result.entryName}. Password field is not visible yet.`;
  }

  if (result.filledPassword) {
    return `Filled password for ${result.entryName}.`;
  }

  return `Filled ${result.entryName}. ${result.filledFields} fields updated.`;
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
        setBackendStatus(false, "Disconnected");
        renderMessage(`Background message failed: ${runtimeError.message}`, "error");
        return;
      }

      if (!response) {
        setBackendStatus(false, "Disconnected");
        renderMessage("No response received from the extension background.", "error");
        return;
      }

      onSuccess(response);
    }
  );
}

function handleSiteMatchFailure(error: string) {
  currentSiteMatches = [];
  updateFillButtonState();

  if (error === "Current tab is not a supported web page.") {
    setSiteVisibility(false);
    renderMessage(error, "error");
    return;
  }

  setSiteVisibility(true);
  renderSite(siteDetails.hostname, "Could not read saved logins for this page.");
  renderMessage(error, "error");
}

function findSiteMatches() {
  if (!hasSupportedPage) {
    currentSiteMatches = [];
    updateFillButtonState();
    return;
  }

  renderSite(siteDetails.hostname, "Checking for a saved login...");

  const message: PopupToBackgroundMessage = {
    type: "termkey.nativeHost.findSiteMatches",
  };

  sendMessage(message, (response) => {
    if (!response.ok) {
      handleSiteMatchFailure(response.error);
      return;
    }

    if (response.response.type !== "site_matches") {
      handleSiteMatchFailure(
        "Native host returned the wrong response type for site matches."
      );
      return;
    }

    currentSiteMatches = response.response.matches;
    setSiteVisibility(true);
    renderSite(
      response.response.siteHostname,
      describeMatches(response.response.matches)
    );
    updateFillButtonState();

    if (response.response.matches.length === 0) {
      renderMessage(`No saved login found for ${response.response.siteHostname}.`);
      return;
    }

    renderMessage(`Ready to fill ${response.response.matches[0].name}.`, "success");
  });
}

function refreshStatus() {
  setBackendStatus(false, "Checking backend");
  renderMessage("Checking TermKey status...");

  const message: PopupToBackgroundMessage = { type: "termkey.nativeHost.status" };
  sendMessage(message, (response) => {
    if (!response.ok) {
      showUnlockSection(true);
      resetMatches("Backend unavailable.");
      setBackendStatus(false, "Disconnected");
      renderMessage(response.error, "error");
      return;
    }

    if (response.response.type !== "status") {
      showUnlockSection(true);
      resetMatches("Status check failed.");
      setBackendStatus(false, "Disconnected");
      renderMessage(
        "Native host returned the wrong response type for status.",
        "error"
      );
      return;
    }

    setBackendStatus(true, "Connected");
    showUnlockSection(response.response.locked);

    if (response.response.locked) {
      resetMatches("Unlock the vault to check for a saved login.");
      renderMessage("Vault locked. Unlock to enable autofill.");
      return;
    }

    renderMessage("Vault unlocked. Checking this site...");
    findSiteMatches();
  });
}

function unlockVault() {
  const password = masterPasswordInput.value;
  if (!password) {
    renderMessage("Enter your master password to unlock the vault.", "error");
    return;
  }

  unlockVaultButton.disabled = true;
  unlockVaultButton.textContent = "Unlocking...";
  renderMessage("Unlocking vault...");

  const message: PopupToBackgroundMessage = {
    type: "termkey.nativeHost.unlock",
    password,
  };

  sendMessage(message, (response) => {
    unlockVaultButton.disabled = false;
    unlockVaultButton.textContent = "Unlock";
    masterPasswordInput.value = "";

    if (!response.ok) {
      renderMessage(`Unlock failed: ${response.error}`, "error");
      return;
    }

    if (response.response.type !== "unlock") {
      renderMessage(
        "Native host returned the wrong response type for unlock.",
        "error"
      );
      return;
    }

    renderMessage("Vault unlocked. Checking this site...", "success");
    refreshStatus();
  });
}

fillButton.addEventListener("click", () => {
  const bestMatch = currentSiteMatches[0];
  if (!bestMatch) {
    renderMessage("No current-site match is available to fill.", "error");
    return;
  }

  fillButton.disabled = true;
  fillButton.textContent = "Filling...";
  renderMessage(`Filling ${bestMatch.name} into the current page...`);

  sendMessage(
    {
      type: "termkey.autofill.fillBestMatch",
      entryId: bestMatch.id,
    },
    (response) => {
      fillButton.textContent = "Fill";
      updateFillButtonState();

      if (!response.ok) {
        renderMessage(`Autofill failed: ${response.error}`, "error");
        return;
      }

      if (response.response.type !== "fill_result") {
        renderMessage(
          "Background returned the wrong response type for autofill.",
          "error"
        );
        return;
      }

      const result: PopupFillResultResponse = response.response;
      renderMessage(formatFillResultMessage(result), "success");
    }
  );
});

unlockVaultButton.addEventListener("click", unlockVault);

masterPasswordInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    unlockVault();
  }
});

updateFillButtonState();
primeCurrentSite();
refreshStatus();
