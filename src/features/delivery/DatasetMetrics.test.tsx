import { describe, expect, it } from "vitest";

import { formatFileSize } from "./DatasetMetrics";

describe("formatFileSize", () => {
  it("mantiene unidades legibles cuando el dataset supera un gigabyte", () => {
    expect(formatFileSize(1023)).toBe("1023 B");
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(1024 ** 2)).toBe("1.0 MB");
    expect(formatFileSize(1024 ** 3)).toBe("1.0 GB");
    expect(formatFileSize(2.5 * 1024 ** 4)).toBe("2.5 TB");
  });
});
