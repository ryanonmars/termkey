declare const chrome: any;

type FillCredentialsMessage = {
  type: "termkey.fillCredentials";
  entry: {
    username: string | null;
    password: string;
  };
};

function isVisibleInput(input: HTMLInputElement) {
  const rect = input.getBoundingClientRect();
  const style = window.getComputedStyle(input);
  return (
    rect.width > 0 &&
    rect.height > 0 &&
    style.visibility !== "hidden" &&
    style.display !== "none" &&
    !input.disabled &&
    !input.readOnly
  );
}

function setInputValue(input: HTMLInputElement, value: string) {
  input.focus();
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

function getInputText(input: HTMLInputElement, attribute: string) {
  return (input.getAttribute(attribute) ?? "").toLowerCase();
}

function isUsernameCompatibleInput(input: HTMLInputElement) {
  const type = (input.getAttribute("type") ?? "text").toLowerCase();
  return (
    type === "text" ||
    type === "email" ||
    type === "search" ||
    input.autocomplete === "username"
  );
}

function getUsernameCandidateScore(input: HTMLInputElement) {
  if (!isVisibleInput(input) || !isUsernameCompatibleInput(input)) {
    return Number.NEGATIVE_INFINITY;
  }

  const type = (input.getAttribute("type") ?? "text").toLowerCase();
  const autocomplete = input.autocomplete.toLowerCase();
  const descriptor = [
    input.name,
    input.id,
    input.placeholder,
    getInputText(input, "aria-label"),
    getInputText(input, "data-testid"),
  ]
    .join(" ")
    .toLowerCase();

  let score = 0;

  if (autocomplete === "username") {
    score += 8;
  }

  if (type === "email") {
    score += 6;
  }

  if (
    /user|email|login|identifier|account|member|customer/.test(descriptor)
  ) {
    score += 4;
  }

  if (/search|coupon|promo|filter/.test(descriptor)) {
    score -= 6;
  }

  if (type === "search") {
    score -= 4;
  }

  return score;
}

function findPasswordInput(): HTMLInputElement | undefined {
  return Array.from(
    document.querySelectorAll<HTMLInputElement>("input[type='password']")
  ).find((input) => isVisibleInput(input));
}

function findUsernameInput(passwordInput: HTMLInputElement) {
  const searchRoot: ParentNode = passwordInput.form ?? document;
  const inputs = Array.from(
    searchRoot.querySelectorAll<HTMLInputElement>("input")
  );
  const passwordIndex = inputs.indexOf(passwordInput);

  for (let index = passwordIndex - 1; index >= 0; index -= 1) {
    const input = inputs[index];
    if (!isVisibleInput(input)) {
      continue;
    }

    if (isUsernameCompatibleInput(input)) {
      return input;
    }
  }

  return undefined;
}

function findStandaloneUsernameInput() {
  const usernameCandidates = Array.from(
    document.querySelectorAll<HTMLInputElement>("input")
  )
    .map((input) => ({
      input,
      score: getUsernameCandidateScore(input),
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

chrome.runtime.onMessage.addListener(
  (
    message: FillCredentialsMessage,
    _sender: unknown,
    sendResponse: (response: unknown) => void
  ) => {
    if (message?.type !== "termkey.fillCredentials") {
      return false;
    }

    let filledFields = 0;
    let filledUsername = false;
    let filledPassword = false;

    const passwordInput = findPasswordInput();
    const usernameInput = passwordInput
      ? findUsernameInput(passwordInput)
      : findStandaloneUsernameInput();

    if (message.entry.username && usernameInput) {
      setInputValue(usernameInput, message.entry.username);
      filledFields += 1;
      filledUsername = true;
    }

    if (passwordInput) {
      setInputValue(passwordInput, message.entry.password);
      filledFields += 1;
      filledPassword = true;
    }

    if (filledFields === 0) {
      if (message.entry.username) {
        sendResponse({
          ok: false,
          error: "No visible username or password field was found on this page.",
        });
        return false;
      }

      sendResponse({
        ok: false,
        error: "No visible password field was found on this page.",
      });
      return false;
    }

    sendResponse({
      ok: true,
      filledFields,
      filledUsername,
      filledPassword,
    });
    return false;
  }
);

console.log("TermKey content script running");
