/**
 * Live-vs-replay parity for image generation packets.
 *
 * Regression test: generated images must appear at the end of generation
 * without requiring a page refresh. The live SSE sequence and the
 * history-replay sequence must produce the same renderable image group.
 */
import { PacketType } from "@/app/app/services/streamingModels";
import {
  createInitialState,
  processPackets,
  GroupedPacket,
} from "@/app/app/message/messageComponents/timeline/hooks/packetProcessor";
import {
  createImageDeltaPacket,
  createMessageStartPacket,
  createPacket,
  createStopPacket,
} from "@/app/app/message/messageComponents/timeline/hooks/__tests__/testHelpers";

function findImageGroup(groups: GroupedPacket[]): GroupedPacket | undefined {
  return groups.find((g) =>
    g.packets.some(
      (p) => p.obj.type === PacketType.IMAGE_GENERATION_TOOL_START
    )
  );
}

describe("image generation live vs replay parity", () => {
  test("live SSE sequence yields a renderable image group with images", () => {
    const state = createInitialState(1);
    const packets = [
      // First text turn.
      createMessageStartPacket({ turn_index: 0 }),
      // Tool turn: a tool_call_debug metadata packet leads the group when the
      // backend runs with INTEGRATION_TESTS_MODE, then START, heartbeats
      // (unknown type on the wire, grouped anyway), FINAL with the image.
      createPacket("tool_call_debug" as PacketType, { turn_index: 1 }),
      createPacket(PacketType.IMAGE_GENERATION_TOOL_START, { turn_index: 1 }),
      createPacket("image_generation_heartbeat" as PacketType, {
        turn_index: 1,
      }),
      createImageDeltaPacket(1, { turn_index: 1 }),
      // Final text turn triggers SECTION_END injection for the tool group.
      createMessageStartPacket({ turn_index: 2 }),
      createStopPacket(),
    ];
    const result = processPackets(state, packets);

    const group = findImageGroup(result.potentialDisplayGroups);
    expect(group).toBeDefined();
    const finalPacket = group!.packets.find(
      (p) => p.obj.type === PacketType.IMAGE_GENERATION_TOOL_DELTA
    );
    expect(finalPacket).toBeDefined();
    expect(
      (finalPacket!.obj as { images: unknown[] }).images
    ).toHaveLength(1);
    // The group is closed so the renderer reports completion.
    expect(
      group!.packets.some((p) => p.obj.type === PacketType.SECTION_END)
    ).toBe(true);
  });

  test("history replay sequence yields the same image group shape", () => {
    const state = createInitialState(1);
    const packets = [
      createMessageStartPacket({ turn_index: 0 }),
      createPacket(PacketType.IMAGE_GENERATION_TOOL_START, { turn_index: 1 }),
      createImageDeltaPacket(1, { turn_index: 1 }),
      createPacket(PacketType.SECTION_END, { turn_index: 1 }),
      createMessageStartPacket({ turn_index: 2 }),
      createStopPacket(),
    ];
    const result = processPackets(state, packets);

    const group = findImageGroup(result.potentialDisplayGroups);
    expect(group).toBeDefined();
    expect(
      (
        group!.packets.find(
          (p) => p.obj.type === PacketType.IMAGE_GENERATION_TOOL_DELTA
        )!.obj as { images: unknown[] }
      ).images
    ).toHaveLength(1);
  });
});
