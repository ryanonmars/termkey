declare const chrome: any;

type FillCredentialsMessage = {
  type: "termkey.fillCredentials";
  entry: {
    username: string | null;
    password: string;
  };
};

type FillGeneratedPasswordMessage = {
  type: "termkey.fillGeneratedPassword";
  password: string;
};

type ContentScriptProbeMessage = {
  type: "termkey.contentScriptProbe";
};

type CaptureVisibleCredentialsMessage = {
  type: "termkey.captureVisibleCredentials";
};

type FillAttemptResult = {
  filledFields: number;
  filledUsername: boolean;
  filledPassword: boolean;
};

const FILL_RETRY_DELAYS_MS = [0, 150, 350, 700] as const;

function sleep(delayMs: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, delayMs);
  });
}

function getInputType(input: HTMLInputElement) {
  return (input.getAttribute("type") ?? "text").toLowerCase();
}

function isVisibleInput(input: HTMLInputElement) {
  const rect = input.getBoundingClientRect();
  const style = window.getComputedStyle(input);

  if (getInputType(input) === "hidden") {
    return false;
  }

  return (
    rect.width > 0 &&
    rect.height > 0 &&
    style.visibility !== "hidden" &&
    style.display !== "none" &&
    style.opacity !== "0" &&
    style.pointerEvents !== "none" &&
    !input.disabled &&
    !input.readOnly &&
    !input.closest("[aria-hidden='true']")
  );
}

function setInputValue(input: HTMLInputElement, value: string) {
  input.focus();

  const prototype = Object.getPrototypeOf(input) as HTMLInputElement;
  const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");

  if (descriptor?.set) {
    descriptor.set.call(input, value);
  } else {
    input.value = value;
  }

  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function getInputText(input: HTMLInputElement, attribute: string) {
  return (input.getAttribute(attribute) ?? "").toLowerCase();
}

function getAutocompleteTokens(input: HTMLInputElement) {
  return input.autocomplete
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

function collectInputElements() {
  const seen = new Set<HTMLInputElement>();

  function visit(root: ParentNode) {
    if (!("querySelectorAll" in root)) {
      return;
    }

    root.querySelectorAll<HTMLInputElement>("input").forEach((input) => {
      seen.add(input);
    });

    root.querySelectorAll<HTMLElement>("*").forEach((element) => {
      if (element.shadowRoot) {
        visit(element.shadowRoot);
      }
    });
  }

  visit(document);
  return Array.from(seen);
}

function getInputLabelText(input: HTMLInputElement) {
  const labels = new Set<string>();

  input.labels?.forEach((label) => {
    labels.add(label.textContent ?? "");
  });

  const wrappingLabel = input.closest("label");
  if (wrappingLabel) {
    labels.add(wrappingLabel.textContent ?? "");
  }

  return Array.from(labels).join(" ").toLowerCase();
}

function getInputDescriptor(input: HTMLInputElement) {
  const form = input.form;

  return [
    input.name,
    input.id,
    input.placeholder,
    input.autocomplete,
    getInputText(input, "aria-label"),
    getInputText(input, "data-testid"),
    getInputText(input, "data-qa"),
    getInputText(input, "data-test"),
    getInputLabelText(input),
    form?.getAttribute("aria-label") ?? "",
    form?.getAttribute("name") ?? "",
    form?.getAttribute("id") ?? "",
  ]
    .join(" ")
    .toLowerCase();
}

function getCandidateRoot(element: HTMLElement | null | undefined) {
  return (
    element?.closest("form, [role='dialog'], dialog, [role='form'], main, section, article") ??
    null
  );
}

function getContextBoost(input: HTMLInputElement) {
  let score = 0;
  const activeElement = document.activeElement;

  if (activeElement === input) {
    score += 10;
  }

  if (!(activeElement instanceof HTMLElement)) {
    return score;
  }

  const activeRoot = getCandidateRoot(activeElement);
  if (activeRoot?.contains(input)) {
    score += 6;
  }

  if (input.form && activeElement instanceof HTMLElement && input.form.contains(activeElement)) {
    score += 6;
  }

  return score;
}

function isUsernameCompatibleInput(input: HTMLInputElement) {
  const type = getInputType(input);
  const autocompleteTokens = getAutocompleteTokens(input);

  return (
    type === "text" ||
    type === "email" ||
    type === "tel" ||
    type === "search" ||
    autocompleteTokens.includes("username") ||
    autocompleteTokens.includes("email")
  );
}

function getUsernameCandidateScore(
  input: HTMLInputElement,
  passwordInput: HTMLInputElement | undefined
) {
  if (!isVisibleInput(input) || !isUsernameCompatibleInput(input)) {
    return Number.NEGATIVE_INFINITY;
  }

  const type = getInputType(input);
  const autocompleteTokens = getAutocompleteTokens(input);
  const descriptor = getInputDescriptor(input);

  let score = 0;

  if (autocompleteTokens.includes("username")) {
    score += 14;
  }

  if (autocompleteTokens.includes("email")) {
    score += 10;
  }

  if (type === "email") {
    score += 8;
  }

  if (type === "tel") {
    score += 5;
  }

  if (
    /user|email|login|identifier|account|member|customer|phone|mobile/.test(
      descriptor
    )
  ) {
    score += 6;
  }

  if (/search|coupon|promo|filter|captcha/.test(descriptor)) {
    score -= 8;
  }

  if (/otp|code|2fa|pass|password|pin/.test(descriptor)) {
    score -= 12;
  }

  if (type === "search") {
    score -= 6;
  }

  if (passwordInput) {
    if (passwordInput.form && input.form === passwordInput.form) {
      score += 10;
    }

    const passwordRoot = getCandidateRoot(passwordInput);
    if (passwordRoot?.contains(input)) {
      score += 6;
    }

    if (input.compareDocumentPosition(passwordInput) & Node.DOCUMENT_POSITION_FOLLOWING) {
      score += 4;
    }
  }

  return score + getContextBoost(input);
}

function getPasswordCandidateScore(input: HTMLInputElement) {
  if (!isVisibleInput(input) || getInputType(input) !== "password") {
    return Number.NEGATIVE_INFINITY;
  }

  const autocompleteTokens = getAutocompleteTokens(input);
  const descriptor = getInputDescriptor(input);
  let score = 0;

  if (autocompleteTokens.includes("current-password")) {
    score += 18;
  }

  if (autocompleteTokens.includes("password")) {
    score += 8;
  }

  if (autocompleteTokens.includes("new-password")) {
    score -= 14;
  }

  if (/pass|password|passcode|pwd|secret/.test(descriptor)) {
    score += 4;
  }

  if (/confirm|confirmation|repeat|verify|re-enter/.test(descriptor)) {
    score -= 14;
  }

  if (/otp|one.?time|2fa|code|search|coupon|promo/.test(descriptor)) {
    score -= 12;
  }

  return score + getContextBoost(input);
}

function findBestPasswordInput(inputs: HTMLInputElement[]) {
  const passwordCandidates = inputs
    .map((input) => ({
      input,
      score: getPasswordCandidateScore(input),
    }))
    .filter(
      (
        candidate
      ): candidate is { input: HTMLInputElement; score: number } =>
        Number.isFinite(candidate.score)
    )
    .sort((left, right) => right.score - left.score);

  return passwordCandidates[0]?.input;
}

function findBestUsernameInput(
  inputs: HTMLInputElement[],
  passwordInput: HTMLInputElement | undefined
) {
  const usernameCandidates = inputs
    .map((input) => ({
      input,
      score: getUsernameCandidateScore(input, passwordInput),
    }))
    .filter(
      (
        candidate
      ): candidate is { input: HTMLInputElement; score: number } =>
        Number.isFinite(candidate.score) && candidate.score > 0
    )
    .sort((left, right) => right.score - left.score);

  return usernameCandidates[0]?.input;
}

function sharesCandidateContext(
  left: HTMLInputElement,
  right: HTMLInputElement | undefined
) {
  if (!right) {
    return false;
  }

  if (left.form && right.form && left.form === right.form) {
    return true;
  }

  const leftRoot = getCandidateRoot(left);
  const rightRoot = getCandidateRoot(right);
  return Boolean(leftRoot && rightRoot && leftRoot === rightRoot);
}

function getGeneratedPasswordCandidateScore(input: HTMLInputElement) {
  if (!isVisibleInput(input) || getInputType(input) !== "password") {
    return Number.NEGATIVE_INFINITY;
  }

  const autocompleteTokens = getAutocompleteTokens(input);
  const descriptor = getInputDescriptor(input);
  let score = 0;

  if (autocompleteTokens.includes("new-password")) {
    score += 20;
  }

  if (/new|create|choose|set|signup|sign.?up|register/.test(descriptor)) {
    score += 10;
  }

  if (/confirm|confirmation|repeat|verify|re-enter/.test(descriptor)) {
    score -= 8;
  }

  if (autocompleteTokens.includes("current-password")) {
    score -= 18;
  }

  if (/current|old|existing/.test(descriptor)) {
    score -= 18;
  }

  if (/otp|one.?time|2fa|code|search|coupon|promo/.test(descriptor)) {
    score -= 12;
  }

  return score + getContextBoost(input);
}

function getConfirmationPasswordScore(
  input: HTMLInputElement,
  primaryPasswordInput: HTMLInputElement | undefined
) {
  if (
    !isVisibleInput(input) ||
    getInputType(input) !== "password" ||
    input === primaryPasswordInput
  ) {
    return Number.NEGATIVE_INFINITY;
  }

  const autocompleteTokens = getAutocompleteTokens(input);
  const descriptor = getInputDescriptor(input);
  let score = 0;

  if (/confirm|confirmation|repeat|verify|re-enter|match/.test(descriptor)) {
    score += 18;
  }

  if (autocompleteTokens.includes("new-password")) {
    score += 8;
  }

  if (sharesCandidateContext(input, primaryPasswordInput)) {
    score += 8;
  }

  if (
    primaryPasswordInput &&
    input.compareDocumentPosition(primaryPasswordInput) &
      Node.DOCUMENT_POSITION_PRECEDING
  ) {
    score += 6;
  }

  if (autocompleteTokens.includes("current-password")) {
    score -= 18;
  }

  if (/current|old|existing/.test(descriptor)) {
    score -= 18;
  }

  if (/otp|one.?time|2fa|code|search|coupon|promo/.test(descriptor)) {
    score -= 12;
  }

  return score + getContextBoost(input);
}

function findGeneratedPasswordTargets(inputs: HTMLInputElement[]) {
  const passwordCandidates = inputs
    .map((input) => ({
      input,
      score: getGeneratedPasswordCandidateScore(input),
    }))
    .filter(
      (
        candidate
      ): candidate is { input: HTMLInputElement; score: number } =>
        Number.isFinite(candidate.score)
    )
    .sort((left, right) => right.score - left.score);

  const primaryPasswordInput = passwordCandidates[0]?.input;
  if (!primaryPasswordInput) {
    return {};
  }

  const confirmationCandidates = inputs
    .map((input) => ({
      input,
      score: getConfirmationPasswordScore(input, primaryPasswordInput),
    }))
    .filter(
      (
        candidate
      ): candidate is { input: HTMLInputElement; score: number } =>
        Number.isFinite(candidate.score)
    )
    .sort((left, right) => right.score - left.score);

  let confirmationPasswordInput = confirmationCandidates[0]?.input;
  if (
    !confirmationPasswordInput &&
    passwordCandidates.length > 1 &&
    sharesCandidateContext(passwordCandidates[1].input, primaryPasswordInput)
  ) {
    confirmationPasswordInput = passwordCandidates[1].input;
  }

  return {
    primaryPasswordInput,
    confirmationPasswordInput,
    usernameInput: findBestUsernameInput(inputs, primaryPasswordInput),
  };
}

function fillVisibleCredentials(
  message: FillCredentialsMessage
): FillAttemptResult {
  const inputs = collectInputElements();
  const passwordInput = findBestPasswordInput(inputs);
  const usernameInput = message.entry.username
    ? findBestUsernameInput(inputs, passwordInput)
    : undefined;

  let filledUsername = false;
  let filledPassword = false;

  if (message.entry.username && usernameInput) {
    setInputValue(usernameInput, message.entry.username);
    filledUsername = true;
  }

  if (passwordInput) {
    setInputValue(passwordInput, message.entry.password);
    filledPassword = true;
  }

  return {
    filledFields: Number(filledUsername) + Number(filledPassword),
    filledUsername,
    filledPassword,
  };
}

async function fillCredentials(message: FillCredentialsMessage) {
  let filledUsername = false;
  let filledPassword = false;

  for (const delayMs of FILL_RETRY_DELAYS_MS) {
    if (delayMs > 0) {
      await sleep(delayMs);
    }

    const attempt = fillVisibleCredentials(message);
    filledUsername ||= attempt.filledUsername;
    filledPassword ||= attempt.filledPassword;

    if (filledPassword) {
      break;
    }
  }

  const filledFields = Number(filledUsername) + Number(filledPassword);
  if (filledFields === 0) {
    if (message.entry.username) {
      return {
        ok: false,
        error: "No visible username or password field was found on this page.",
      };
    }

    return {
      ok: false,
      error: "No visible password field was found on this page.",
    };
  }

  return {
    ok: true,
    filledFields,
    filledUsername,
    filledPassword,
  };
}

function captureVisibleCredentials() {
  const inputs = collectInputElements();
  const passwordInput = findBestPasswordInput(inputs);

  if (!passwordInput) {
    return {
      ok: false,
      error: "No visible password field was found on this page.",
    };
  }

  if (!passwordInput.value) {
    return {
      ok: false,
      error: "Type your password into the page before saving this login.",
    };
  }

  const usernameInput = findBestUsernameInput(inputs, passwordInput);
  const username = usernameInput?.value.trim() || null;

  return {
    ok: true,
    username,
    password: passwordInput.value,
  };
}

function fillGeneratedPassword(message: FillGeneratedPasswordMessage) {
  const inputs = collectInputElements();
  const {
    primaryPasswordInput,
    confirmationPasswordInput,
    usernameInput,
  } = findGeneratedPasswordTargets(inputs);

  if (!primaryPasswordInput) {
    return {
      ok: false,
      error:
        "No visible signup password field was found on this page. Open the account creation form first.",
    };
  }

  setInputValue(primaryPasswordInput, message.password);

  let filledPasswordFields = 1;
  if (
    confirmationPasswordInput &&
    confirmationPasswordInput !== primaryPasswordInput
  ) {
    setInputValue(confirmationPasswordInput, message.password);
    filledPasswordFields += 1;
  }

  return {
    ok: true,
    username: usernameInput?.value.trim() || null,
    filledPasswordFields,
  };
}

chrome.runtime.onMessage.addListener(
  (
    message:
      | FillCredentialsMessage
      | FillGeneratedPasswordMessage
      | ContentScriptProbeMessage
      | CaptureVisibleCredentialsMessage,
    _sender: unknown,
    sendResponse: (response: unknown) => void
  ) => {
    if (message?.type === "termkey.contentScriptProbe") {
      sendResponse({ ok: true });
      return true;
    }

    if (message?.type === "termkey.captureVisibleCredentials") {
      sendResponse(captureVisibleCredentials());
      return true;
    }

    if (message?.type === "termkey.fillGeneratedPassword") {
      sendResponse(fillGeneratedPassword(message));
      return true;
    }

    if (message?.type !== "termkey.fillCredentials") {
      return false;
    }

    void fillCredentials(message)
      .then(sendResponse)
      .catch((error) => {
        sendResponse({
          ok: false,
          error:
            error instanceof Error
              ? error.message
              : "Content script failed while filling the page.",
        });
      });

    return true;
  }
);

console.log("TermKey content script running");
