import React, { useRef, useState } from "react";
import { act, screen } from "@testing-library/react";
import { render } from "@tests/setup/test-utils";
import MicrophoneButton from "@/sections/input/MicrophoneButton";

// Drives the recorder from the test instead of a real microphone.
let emitLiveTranscript: (text: string) => void = () => {};

jest.mock("@/hooks/useVoiceRecorder", () => {
  const React = jest.requireActual<typeof import("react")>("react");
  return {
    useVoiceRecorder: () => {
      const [liveTranscript, setLiveTranscript] = React.useState("");
      emitLiveTranscript = setLiveTranscript;
      return {
        isRecording: true,
        isProcessing: false,
        isMuted: false,
        error: null,
        liveTranscript,
        audioLevel: 0,
        startRecording: jest.fn(),
        stopRecording: jest.fn(async () => null),
        setMuted: jest.fn(),
      };
    },
  };
});

jest.mock("@/providers/VoiceModeProvider", () => ({
  useVoiceMode: () => ({
    isTTSPlaying: false,
    isTTSLoading: false,
    isAwaitingAutoPlaybackStart: false,
    manualStopCount: 0,
  }),
}));

interface HarnessProps {
  onTranscription: (text: string) => void;
}

// Mirrors AppInputBar: a send clears the input, then silences the recorder.
function Harness({ onTranscription }: HarnessProps) {
  const suppressTranscriptRef = useRef<(() => void) | null>(null);
  const [, forceRender] = useState(0);

  return (
    <>
      <button
        onClick={() => {
          suppressTranscriptRef.current?.();
          forceRender((n) => n + 1);
        }}
      >
        send
      </button>
      <MicrophoneButton
        onTranscription={onTranscription}
        suppressTranscriptRef={suppressTranscriptRef}
      />
    </>
  );
}

describe("MicrophoneButton transcript suppression", () => {
  it("stops feeding the input once the message has been sent", () => {
    const onTranscription = jest.fn();
    render(<Harness onTranscription={onTranscription} />);

    act(() => emitLiveTranscript("hello"));
    expect(onTranscription).toHaveBeenCalledWith("hello");

    act(() => {
      screen.getByText("send").click();
    });
    onTranscription.mockClear();

    // A transcript still in flight must not refill the cleared input.
    act(() => emitLiveTranscript("hello there"));
    expect(onTranscription).not.toHaveBeenCalled();
  });
});
