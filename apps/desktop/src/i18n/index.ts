/** i18n: every shell/settings string lives in resource files. English (en-US) is the default.
 *  Community translations go in src/i18n/<lang>.json with the same keys. */
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";

void i18n.use(initReactI18next).init({
  resources: { "en-US": { translation: en } },
  lng: "en-US",
  fallbackLng: "en-US",
  interpolation: { escapeValue: false }, // React already escapes
  returnNull: false,
});

export default i18n;
