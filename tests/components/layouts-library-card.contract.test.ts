import { mountSuspended } from "@nuxt/test-utils/runtime";
import { describe, expect, it } from "vitest";

import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";

describe("LayoutsLibraryCard", () => {
  it("emits its public actions from the rendered controls", async () => {
    const entries = [
      { id: "user:ivan-k", name: "Ivan K's" },
      { id: "user:nav", name: "Nav", description: "Navigation" },
    ];

    const wrapper = await mountSuspended(LayoutsLibraryCard, {
      props: {
        entries,
        currentLayoutId: "user:nav",
        currentLayoutDescription: "Changed description",
        isLayoutDirty: true,
        applying: "",
        applyError: null,
        libraryError: null,
        layoutsDir: "/tmp/layouts",
        layoutMode: "manual",
        autoIncludedIds: new Set(),
      },
    });

    const buttons = wrapper.findAll("button");
    const listItems = wrapper.findAll("li");

    await buttons[0]?.trigger("click");
    await buttons[1]?.trigger("click");
    await buttons[2]?.trigger("click");
    await buttons[3]?.trigger("click");
    await buttons[4]?.trigger("click");
    
    // entries[0] Edit and Delete
    await buttons[5]?.trigger("click");
    await buttons[6]?.trigger("click");

    // Click list items for Load (requestApplyEntry)
    await listItems[0]?.trigger("click");
    await listItems[1]?.trigger("click");

    expect(wrapper.emitted("createFromEmpty")).toHaveLength(1);
    expect(wrapper.emitted("createFromIvanK")).toHaveLength(1);
    expect(wrapper.emitted("saveAs")).toHaveLength(1);
    expect(wrapper.emitted("saveCurrent")).toHaveLength(1);
    expect(wrapper.emitted("requestReset")).toHaveLength(1);
    expect(wrapper.emitted("requestApplyEntry")).toEqual([
      [entries[0]],
      [entries[1]],
    ]);
    expect(wrapper.emitted("requestEdit")).toEqual([[entries[0]]]);
    expect(wrapper.emitted("requestDelete")).toEqual([[entries[0]]]);
  });
});
