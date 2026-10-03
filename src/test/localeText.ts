/**
 * QA-05: matches a number written as in en-US ("66.7%", "2.3 GiB") whatever
 * the regional setting of the machine: the decimal point may be a comma and a
 * space may come before "%". Thousands separators are not used in these texts.
 */
export function localeText(text: string, { exact = false } = {}): RegExp {
  const pattern = text
    .replace(/[\\^$*+?()[\]{}|]/g, "\\$&")
    .replace(/(\d)\.(\d)/g, "$1[.,]$2")
    .replace(/%/g, "\\s?%");
  return new RegExp(exact ? `^${pattern}$` : pattern);
}
