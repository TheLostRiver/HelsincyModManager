import { useContext } from "react";
import { ColorSchemeContext } from "./ColorSchemeProvider";

export function useColorScheme() {
  const value = useContext(ColorSchemeContext);

  if (value === null) {
    throw new Error("useColorScheme must be used within ColorSchemeProvider");
  }

  return value;
}
