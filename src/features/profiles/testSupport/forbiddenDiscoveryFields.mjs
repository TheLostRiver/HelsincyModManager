export const forbiddenDiscoveryFields = new RegExp(
  [
    "steam" + "Id64",
    "account" + "Id",
    "raw" + "Path",
    "full" + "Path",
    "x" + "ml",
    "profile" + "Url",
  ].join("|"),
  "i",
);
