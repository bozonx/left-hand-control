import { mountSuspended } from "@nuxt/test-utils/runtime";
import { describe, expect, it } from "vitest";

import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";

describe("LayoutsLibraryCard", () => {
  it("emits its public actions from the rendered controls", async () => {
    const entries = [
      { id: "builtin:ivank", name: "Ivan K's", builtin: true },
      { id: "user:nav", name: "Nav", builtin: false },
    ];

    const wrapper = await mountSuspended(LayoutsLibraryCard, {
      props: {
        entries,
        currentLayoutId: "user:nav",
        isLayoutDirty: true,
        applying: "",
        applyError: null,
        libraryError: null,
        layoutsDir: "/tmp/layouts",
      },
    });

    const buttons = wrapper.findAll("button");

    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    await buttons[2]?.trigger("click");
    await buttons[3]?.trigger("click");
    await buttons[4]?.trigger("click");
    await buttons[5]?.trigger("click");

    expect(wrapper.emitted("saveAs")).toHaveLength(1);
    expect(wrapper.emitted("saveCurrent")).toHaveLength(1);
    expect(wrapper.emitted("requestApplyEntry")).toEqual([
      [entries[0]],
      [entries[1]],
    ]);
    expect(wrapper.emitted("requestDelete")).toEqual([[entries[1]]]);
    expect(wrapper.emitted("requestApplyEmpty")).toHaveLength(1);
  });
});
