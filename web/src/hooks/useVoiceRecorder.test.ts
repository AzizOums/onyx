import { collapseRepeatedSentences } from "@/hooks/useVoiceRecorder";

describe("collapseRepeatedSentences", () => {
  it("leaves normal speech untouched", () => {
    const text = "Euh... Je veux que tu génères une image. Surprends-moi.";
    expect(collapseRepeatedSentences(text)).toBe(text);
  });

  it("keeps a legitimate double-up", () => {
    const text = "Go. Go. And then stop.";
    expect(collapseRepeatedSentences(text)).toBe(text);
  });

  it("collapses whisper hallucination loops", () => {
    const spam = Array(10).fill("Продолжение следует").join(". ") + ".";
    expect(collapseRepeatedSentences(spam)).toBe(
      "Продолжение следует. Продолжение следует."
    );
  });

  it("collapses repeats across mixed sentences independently", () => {
    const text =
      "Hello world. Thank you. Thank you. Thank you. Thank you. Goodbye.";
    expect(collapseRepeatedSentences(text)).toBe(
      "Hello world. Thank you. Thank you. Goodbye."
    );
  });

  it("handles empty text", () => {
    expect(collapseRepeatedSentences("")).toBe("");
  });
});
