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
    const type = (input.getAttribute("type") ?? "text").toLowerCase();
    if (!isVisibleInput(input)) {
      continue;
    }

    if (
      type === "text" ||
      type === "email" ||
      type === "search" ||
      input.autocomplete === "username"
    ) {
      return input;
    }
  }

  return undefined;
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

    const passwordInput = findPasswordInput();
    if (!passwordInput) {
      sendResponse({
        ok: false,
        error: "No visible password field was found on this page.",
      });
      return false;
    }

    let filledFields = 0;
    if (message.entry.username) {
      const usernameInput = findUsernameInput(passwordInput);
      if (usernameInput) {
        setInputValue(usernameInput, message.entry.username);
        filledFields += 1;
      }
    }

    setInputValue(passwordInput, message.entry.password);
    filledFields += 1;

    sendResponse({ ok: true, filledFields });
    return false;
  }
);

console.log("TermKey content script running");
