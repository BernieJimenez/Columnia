import { configure } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

// App.test renders the whole shell; with every worker busy a findBy can
// need more than the default 1 s and fail intermittently.
configure({ asyncUtilTimeout: 3_000 });

// QA-05: `COLUMNIA_TEST_LOCALE=es-ES npx vitest run` runs the suite as on a
// machine with that regional setting, so assertions cannot depend on the
// separators of the machine that runs them.
const forcedLocale = process.env.COLUMNIA_TEST_LOCALE;
if (forcedLocale) {
  const NativeNumberFormat = Intl.NumberFormat;
  const NativeDateTimeFormat = Intl.DateTimeFormat;
  const withLocale = (locales: Intl.LocalesArgument | undefined) => locales ?? forcedLocale;
  Intl.NumberFormat = Object.assign(
    function NumberFormat(locales?: Intl.LocalesArgument, options?: Intl.NumberFormatOptions) {
      return new NativeNumberFormat(withLocale(locales), options);
    },
    NativeNumberFormat,
  ) as typeof Intl.NumberFormat;
  Intl.DateTimeFormat = Object.assign(
    function DateTimeFormat(locales?: Intl.LocalesArgument, options?: Intl.DateTimeFormatOptions) {
      return new NativeDateTimeFormat(withLocale(locales), options);
    },
    NativeDateTimeFormat,
  ) as typeof Intl.DateTimeFormat;
  const nativeNumberToLocale = Number.prototype.toLocaleString;
  Number.prototype.toLocaleString = function toLocaleString(locales?: Intl.LocalesArgument, options?: Intl.NumberFormatOptions) {
    return nativeNumberToLocale.call(this, withLocale(locales), options);
  };
  const nativeDateToLocale = Date.prototype.toLocaleString;
  Date.prototype.toLocaleString = function toLocaleString(locales?: Intl.LocalesArgument, options?: Intl.DateTimeFormatOptions) {
    return nativeDateToLocale.call(this, withLocale(locales), options);
  };
}
