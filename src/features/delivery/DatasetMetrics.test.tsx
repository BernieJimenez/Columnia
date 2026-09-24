import { describe, expect, it } from "vitest";

import { formatFileSize } from "./DatasetMetrics";

describe("formatFileSize", () => {
  it("mantiene unidades legibles cuando el dataset supera un gigabyte", () => {
    expect(formatFileSize(1023)).toBe("1023 B");
    expect(formatFileSize(1024)).toMatch(/^1[.,]0 KiB$/);
    expect(formatFileSize(1024 ** 2)).toMatch(/^1[.,]0 MiB$/);
    expect(formatFileSize(1024 ** 3)).toMatch(/^1[.,]0 GiB$/);
    expect(formatFileSize(2.5 * 1024 ** 4)).toMatch(/^2[.,]5 TiB$/);
  });
});
