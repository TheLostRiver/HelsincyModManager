import { useContext } from "react";
import { I18nContext } from "./I18nProvider";

export function useI18n() {
  const value = useContext(I18nContext);

  if (value === null) {
    throw new Error("useI18n must be used within I18nProvider");
  }

  return value;
}
