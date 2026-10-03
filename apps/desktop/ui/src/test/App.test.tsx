import { describe, it, expect } from "vitest";

describe("Baseline App Smoke Test", () => {
  it("passes initial sanity check", () => {
    expect(1 + 1).toBe(2);
  });
});
