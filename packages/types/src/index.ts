export type NativeHostRequest =
  | {
      type: "ping";
    }
  | {
      type: "status";
    }
  | {
      type: "get_autofill_entry";
      id: string;
      password?: string;
      secondaryPassword?: string;
    }
  | {
      type: "find_site_matches";
      url: string;
    }
  | {
      type: "list_entries";
    }
  | {
      type: "unlock";
      password: string;
    };

export type NativeHostPongResponse = {
  type: "pong";
  app: "termkey";
  version: string;
};

export type NativeHostStatusResponse = {
  type: "status";
  app: "termkey";
  version: string;
  vaultPath: string;
  vaultExists: boolean;
  firstRunComplete: boolean;
  recoveryConfigured: boolean;
  locked: boolean;
};

export type NativeHostEntrySummary = {
  id: string;
  name: string;
  secretType: string;
  network: string;
  hasSecondaryPassword: boolean;
  publicAddress: string | null;
  username: string | null;
  url: string | null;
};

export type NativeHostSiteMatch = {
  id: string;
  name: string;
  username: string | null;
  url: string | null;
  matchType: "exact_origin" | "exact_host" | "subdomain";
  hasSecondaryPassword: boolean;
};

export type NativeHostAutofillEntry = {
  id: string;
  name: string;
  username: string | null;
  password: string;
  url: string | null;
};

export type PopupFillResultResponse = {
  type: "fill_result";
  entryName: string;
  filledFields: number;
  filledUsername: boolean;
  filledPassword: boolean;
};

export type NativeHostResponse =
  | NativeHostPongResponse
  | NativeHostStatusResponse
  | {
      type: "autofill_entry";
      entry: NativeHostAutofillEntry;
    }
  | {
      type: "site_matches";
      siteUrl: string;
      siteOrigin: string;
      siteHostname: string;
      matches: NativeHostSiteMatch[];
    }
  | {
      type: "list_entries";
      entries: NativeHostEntrySummary[];
    }
  | {
      type: "unlock";
      unlocked: true;
    }
  | {
      type: "error";
      message: string;
    };

export type PopupToBackgroundMessage =
  | {
      type: "termkey.nativeHost.ping";
    }
  | {
      type: "termkey.nativeHost.status";
    }
  | {
      type: "termkey.nativeHost.findSiteMatches";
    }
  | {
      type: "termkey.autofill.fillSelectedMatch";
      entryId: string;
      password?: string;
      secondaryPassword?: string;
    }
  | {
      type: "termkey.nativeHost.unlock";
      password: string;
    };

export type PopupToBackgroundResponse =
  | {
      ok: true;
      response: NativeHostResponse | PopupFillResultResponse;
    }
  | {
      ok: false;
      error: string;
    };
