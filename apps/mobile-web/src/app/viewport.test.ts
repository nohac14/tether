import { describe, expect, it } from "vitest";
import { chooseViewport } from "./viewport";

describe("mobile visual viewport", () => {
  it("uses the visible size and offsets when the viewport is cropped", () => {
    const visible = { width: 390, height: 355, offsetLeft: 0, offsetTop: 104 };
    expect(chooseViewport(visible, 390, 844)).toEqual(visible);
  });

  it("uses the layout viewport when the visual API is missing or temporarily empty", () => {
    const landscape = { width: 844, height: 390, offsetLeft: 0, offsetTop: 0 };
    expect(chooseViewport(null, 844, 390)).toEqual(landscape);
    expect(chooseViewport({ ...landscape, height: 0 }, 844, 390)).toEqual(landscape);
    expect(chooseViewport({ ...landscape, width: Number.NaN }, 844, 390)).toEqual(landscape);
  });

  it("keeps non-finite fallbacks and negative overscroll out of CSS", () => {
    expect(chooseViewport(null, Number.NaN, -2)).toEqual({
      width: 1,
      height: 1,
      offsetTop: 0,
      offsetLeft: 0,
    });
    expect(
      chooseViewport(
        {
          width: 390,
          height: 844,
          offsetLeft: -1,
          offsetTop: -8,
        },
        390,
        844,
      ),
    ).toEqual({
      width: 390,
      height: 844,
      offsetLeft: 0,
      offsetTop: 0,
    });
  });
});
