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
      gap: 12px;
      padding: 14px;
    }

    .header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
    }

    .eyebrow {
      margin: 0;
      color: #7dd3fc;
      font-size: 13px;
      font-weight: 600;
      letter-spacing: -0.03em;
    }

    .connection {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 9px;
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
      padding: 12px;
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
      margin-bottom: 4px;
      color: #7dd3fc;
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .site-hostname {
      display: block;
      font-size: 15px;
      font-weight: 600;
      line-height: 1.3;
      word-break: break-word;
    }

    .site-summary {
      display: block;
      margin-top: 3px;
      color: #93a4b8;
      font-size: 12px;
      line-height: 1.4;
    }

    .match-picker {
      display: grid;
      gap: 10px;
      margin-top: 14px;
      padding-top: 14px;
      border-top: 1px solid rgba(148, 163, 184, 0.14);
    }

    .match-picker[hidden] {
      display: none;
    }

    .match-picker-header {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: 12px;
    }

    .match-list-label {
      color: #7dd3fc;
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .match-list-summary {
      color: #93a4b8;
      font-size: 12px;
    }

    .match-list {
      display: grid;
      gap: 8px;
    }

    .match-option {
      display: grid;
      gap: 8px;
      width: 100%;
      padding: 12px;
      border: 1px solid rgba(148, 163, 184, 0.16);
      border-radius: 14px;
      background: rgba(2, 6, 23, 0.52);
      color: #f8fafc;
      text-align: left;
      transition:
        border-color 140ms ease,
        background 140ms ease,
        transform 140ms ease;
    }

    .match-option:hover {
      transform: translateY(-1px);
      border-color: rgba(125, 211, 252, 0.4);
      background: rgba(15, 23, 42, 0.82);
    }

    .match-option-main {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 10px;
    }

    .match-option-name {
      font-size: 14px;
      font-weight: 600;
      line-height: 1.3;
    }

    .match-option-detail {
      display: block;
      margin-top: 4px;
      color: #93a4b8;
      font-size: 12px;
      line-height: 1.4;
      word-break: break-word;
    }

    .match-option-side {
      display: grid;
      justify-items: end;
      flex-shrink: 0;
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

    .match-fill-button {
      min-width: 92px;
      padding: 8px 12px;
      font-size: 12px;
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

    .password-stack {
      display: grid;
      gap: 10px;
    }

    .secondary-password-group[hidden] {
      display: none;
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
      </div>
      <div class="connection">
        <span id="backend-dot" class="status-dot" aria-hidden="true"></span>
        <span id="backend-label">Checking backend</span>
      </div>
    </header>

    <section id="site-panel" class="panel">
      <div class="site-row">
        <div>
          <span class="site-label">Site</span>
          <strong id="site-hostname" class="site-hostname">Waiting for page...</strong>
          <span id="site-summary" class="site-summary">Checking the current tab.</span>
        </div>
        <button id="fill-best-match" class="fill-button" disabled>Fill</button>
      </div>
      <div id="match-picker" class="match-picker" hidden>
        <div class="match-picker-header">
          <span class="match-list-label">Saved logins</span>
          <span id="match-list-summary" class="match-list-summary"></span>
        </div>
        <div
          id="match-list"
          class="match-list"
          role="listbox"
          aria-label="Saved logins for the current site"
        ></div>
      </div>
    </section>

    <section id="unlock-section" class="panel">
      <label>
        <span id="password-panel-label" class="unlock-label">Master password</span>
        <div class="unlock-row">
          <div class="password-stack">
            <input
              id="master-password"
              class="password-input"
              type="password"
              placeholder="Enter your master password"
              autocomplete="current-password"
            />
            <div id="secondary-password-group" class="secondary-password-group" hidden>
              <input
                id="secondary-password"
                class="password-input"
                type="password"
                placeholder="Enter the secondary password"
                autocomplete="off"
              />
            </div>
          </div>
          <button id="unlock-vault" class="unlock-button">Authenticate</button>
        </div>
      </label>
      <p id="password-panel-hint" class="hint">Your password is only used for this fill request.</p>
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
const secondaryPasswordInputEl =
  document.querySelector<HTMLInputElement>("#secondary-password");
const statusEl = document.querySelector<HTMLParagraphElement>("#native-host-status");
const siteHostnameEl =
  document.querySelector<HTMLSpanElement>("#site-hostname");
const siteSummaryEl =
  document.querySelector<HTMLSpanElement>("#site-summary");
const matchPickerEl = document.querySelector<HTMLElement>("#match-picker");
const matchListSummaryEl =
  document.querySelector<HTMLSpanElement>("#match-list-summary");
const matchListEl = document.querySelector<HTMLElement>("#match-list");
const passwordPanelLabelEl =
  document.querySelector<HTMLSpanElement>("#password-panel-label");
const passwordPanelHintEl =
  document.querySelector<HTMLParagraphElement>("#password-panel-hint");
const secondaryPasswordGroupEl =
  document.querySelector<HTMLElement>("#secondary-password-group");

if (
  !backendDotEl ||
  !backendLabelEl ||
  !sitePanelEl ||
  !fillBestMatchButton ||
  !unlockSectionEl ||
  !unlockButton ||
  !passwordInput ||
  !secondaryPasswordInputEl ||
  !statusEl ||
  !siteHostnameEl ||
  !siteSummaryEl ||
  !matchPickerEl ||
  !matchListSummaryEl ||
  !matchListEl ||
  !passwordPanelLabelEl ||
  !passwordPanelHintEl ||
  !secondaryPasswordGroupEl
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
const secondaryPasswordInput = secondaryPasswordInputEl;
const statusMessage = statusEl;
const siteHostname = siteHostnameEl;
const siteSummary = siteSummaryEl;
const matchPicker = matchPickerEl;
const matchListSummary = matchListSummaryEl;
const matchList = matchListEl;
const passwordPanelLabel = passwordPanelLabelEl;
const passwordPanelHint = passwordPanelHintEl;
const secondaryPasswordGroup = secondaryPasswordGroupEl;

let currentSiteMatches: NativeHostSiteMatch[] = [];
let pendingFillMatch: NativeHostSiteMatch | null = null;
let fillingEntryId: string | null = null;
let backendConnected = false;
let vaultExists = true;
let hasSupportedPage = true;
let siteDetails = {
  hostname: "Waiting for page...",
  summary: "Checking the current tab.",
};

function setSiteVisibility(visible: boolean) {
  hasSupportedPage = visible;
  sitePanel.hidden = !visible;
  if (!visible) {
    setCurrentSiteMatches([]);
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
  const singleMatch = currentSiteMatches.length === 1;

  fillButton.hidden = !singleMatch;
  fillButton.disabled =
    !hasSupportedPage ||
    !backendConnected ||
    !singleMatch ||
    fillingEntryId !== null;
  fillButton.textContent = fillingEntryId !== null ? "Filling..." : "Fill";
}

function resetMatches(summaryText: string) {
  setCurrentSiteMatches([]);
  renderSite(siteDetails.hostname, summaryText);
  updateFillButtonState();
}

function renderPasswordPrompt() {
  const activeMatch = pendingFillMatch;
  unlockSection.hidden = activeMatch === null;

  if (!activeMatch) {
    passwordPanelLabel.textContent = "Master password";
    passwordPanelHint.textContent = "Your password is only used for this fill request.";
    masterPasswordInput.disabled = false;
    masterPasswordInput.value = "";
    secondaryPasswordInput.disabled = false;
    secondaryPasswordInput.value = "";
    secondaryPasswordGroup.hidden = true;
    unlockVaultButton.disabled = true;
    unlockVaultButton.textContent = "Authenticate";
    return;
  }

  passwordPanelLabel.textContent = `Master password for ${activeMatch.name}`;
  passwordPanelHint.textContent = activeMatch.hasSecondaryPassword
    ? `Used only for this fill request on ${siteDetails.hostname}. This entry also requires its secondary password.`
    : `Used only for this fill request on ${siteDetails.hostname}.`;
  masterPasswordInput.disabled = fillingEntryId !== null;
  secondaryPasswordInput.disabled = fillingEntryId !== null;
  secondaryPasswordGroup.hidden = !activeMatch.hasSecondaryPassword;
  unlockVaultButton.disabled = !backendConnected || fillingEntryId !== null;
  unlockVaultButton.textContent =
    fillingEntryId === activeMatch.id ? "Authenticating..." : "Authenticate";
}

function describeMatches(matches: NativeHostSiteMatch[]) {
  if (matches.length === 0) {
    return "No saved login found for this site.";
  }

  if (matches.length === 1) {
    const match = matches[0];
    const details = [match.name];

    if (match.username) {
      details.push(match.username);
    }

    return details.join(" • ");
  }

  return `${matches.length} saved logins found. Choose one to fill.`;
}

function formatMatchDetail(match: NativeHostSiteMatch) {
  const suffix = match.hasSecondaryPassword
    ? " • Secondary password required"
    : "";

  if (match.username) {
    return `${match.username}${suffix}`;
  }

  if (match.url) {
    return `${match.url}${suffix}`;
  }

  return suffix ? `No username saved${suffix}` : "No username saved";
}

function renderMatchPicker() {
  const multipleMatches = currentSiteMatches.length > 1;
  matchPicker.hidden = !multipleMatches;
  matchList.replaceChildren();

  if (!multipleMatches) {
    matchListSummary.textContent = "";
    return;
  }

  matchListSummary.textContent = `${currentSiteMatches.length} matches`;

  const fragment = document.createDocumentFragment();

  currentSiteMatches.forEach((match) => {
    const option = document.createElement("div");
    option.className = "match-option";

    const main = document.createElement("span");
    main.className = "match-option-main";

    const content = document.createElement("span");

    const name = document.createElement("span");
    name.className = "match-option-name";
    name.textContent = match.name;

    const detail = document.createElement("span");
    detail.className = "match-option-detail";
    detail.textContent = formatMatchDetail(match);

    const side = document.createElement("span");
    side.className = "match-option-side";

    const actionButton = document.createElement("button");
    actionButton.type = "button";
    actionButton.className = "fill-button match-fill-button";
    actionButton.disabled = !backendConnected || fillingEntryId !== null;
    actionButton.textContent =
      fillingEntryId === match.id
        ? "Filling..."
        : pendingFillMatch?.id === match.id
          ? "Enter password"
          : "Fill";

    content.append(name, detail);
    side.append(actionButton);
    main.append(content, side);
    option.append(main);

    actionButton.addEventListener("click", () => {
      beginFill(match);
    });

    fragment.append(option);
  });

  matchList.append(fragment);
}

function setCurrentSiteMatches(matches: NativeHostSiteMatch[]) {
  currentSiteMatches = matches;
  if (
    pendingFillMatch &&
    !matches.some((match) => match.id === pendingFillMatch?.id)
  ) {
    pendingFillMatch = null;
  }
  renderMatchPicker();
  renderPasswordPrompt();
  updateFillButtonState();
}

function beginFill(match: NativeHostSiteMatch) {
  if (!backendConnected) {
    renderMessage("Reconnect the extension backend before autofill.", "error");
    return;
  }

  pendingFillMatch = match;
  renderMatchPicker();
  renderPasswordPrompt();
  renderMessage(`Enter your master password to fill ${match.name}.`);
  masterPasswordInput.focus();
  masterPasswordInput.select();
}

function submitPendingFill() {
  if (!pendingFillMatch) {
    renderMessage("Choose a saved login before entering your password.", "error");
    return;
  }

  if (!backendConnected) {
    renderMessage("Reconnect the extension backend before autofill.", "error");
    return;
  }

  const password = masterPasswordInput.value;
  if (!password) {
    renderMessage("Enter your master password to fill this login.", "error");
    masterPasswordInput.focus();
    return;
  }

  const secondaryPassword = pendingFillMatch.hasSecondaryPassword
    ? secondaryPasswordInput.value
    : "";
  if (pendingFillMatch.hasSecondaryPassword && !secondaryPassword) {
    renderMessage("Enter the secondary password for this login.", "error");
    secondaryPasswordInput.focus();
    return;
  }

  fillingEntryId = pendingFillMatch.id;
  renderMatchPicker();
  renderPasswordPrompt();
  updateFillButtonState();
  renderMessage(`Filling ${pendingFillMatch.name} into the current page...`);

  sendMessage(
    {
      type: "termkey.autofill.fillSelectedMatch",
      entryId: pendingFillMatch.id,
      password,
      secondaryPassword: secondaryPassword || undefined,
    },
    (response) => {
      fillingEntryId = null;

      if (!response.ok) {
        renderMatchPicker();
        renderPasswordPrompt();
        updateFillButtonState();
        renderMessage(`Autofill failed: ${response.error}`, "error");
        if (pendingFillMatch?.hasSecondaryPassword) {
          secondaryPasswordInput.focus();
          secondaryPasswordInput.select();
        } else {
          masterPasswordInput.focus();
          masterPasswordInput.select();
        }
        return;
      }

      if (response.response.type !== "fill_result") {
        renderMatchPicker();
        renderPasswordPrompt();
        updateFillButtonState();
        renderMessage(
          "Background returned the wrong response type for autofill.",
          "error"
        );
        return;
      }

      pendingFillMatch = null;
      masterPasswordInput.value = "";
      secondaryPasswordInput.value = "";
      renderMatchPicker();
      renderPasswordPrompt();
      updateFillButtonState();

      const result: PopupFillResultResponse = response.response;
      renderMessage(formatFillResultMessage(result), "success");
    }
  );
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
  setCurrentSiteMatches([]);

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
    setCurrentSiteMatches([]);
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

    setCurrentSiteMatches(response.response.matches);
    setSiteVisibility(true);
    renderSite(
      response.response.siteHostname,
      describeMatches(response.response.matches)
    );

    if (response.response.matches.length === 0) {
      renderMessage(`No saved login found for ${response.response.siteHostname}.`);
      return;
    }

    if (response.response.matches.length === 1) {
      renderMessage(
        `Ready to fill ${response.response.matches[0].name}.`,
        "success"
      );
      return;
    }

    renderMessage("Use Fill on the saved login you want for this site.");
  });
}

function refreshStatus() {
  setBackendStatus(false, "Checking backend");
  renderMessage("Checking TermKey status...");

  const message: PopupToBackgroundMessage = { type: "termkey.nativeHost.status" };
  sendMessage(message, (response) => {
    if (!response.ok) {
      pendingFillMatch = null;
      renderPasswordPrompt();
      resetMatches("Backend unavailable.");
      setBackendStatus(false, "Disconnected");
      renderMessage(response.error, "error");
      return;
    }

    if (response.response.type !== "status") {
      pendingFillMatch = null;
      renderPasswordPrompt();
      resetMatches("Status check failed.");
      setBackendStatus(false, "Disconnected");
      renderMessage(
        "Native host returned the wrong response type for status.",
        "error"
      );
      return;
    }

    setBackendStatus(true, "Connected");
    vaultExists = response.response.vaultExists;

    if (!vaultExists) {
      pendingFillMatch = null;
      renderPasswordPrompt();
      resetMatches("Create your vault to save logins for this site.");
      renderMessage("Vault not found. Run `termkey init` first.", "error");
      return;
    }

    renderMessage("Checking this site...");
    findSiteMatches();
  });
}

fillButton.addEventListener("click", () => {
  const singleMatch = currentSiteMatches[0];
  if (!singleMatch) {
    renderMessage("No current-site match is available to fill.", "error");
    return;
  }

  beginFill(singleMatch);
});

unlockVaultButton.addEventListener("click", submitPendingFill);

masterPasswordInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    submitPendingFill();
  }
});

secondaryPasswordInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    submitPendingFill();
  }
});

renderPasswordPrompt();
updateFillButtonState();
primeCurrentSite();
refreshStatus();
