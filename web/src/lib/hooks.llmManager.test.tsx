import { act, renderHook } from "@tests/setup/test-utils";
import { useLlmManager } from "@/lib/hooks";
import { ChatSession, ChatSessionSharedStatus } from "@/app/app/interfaces";
import {
  ReasoningEffortOverride,
  type LLMProviderDescriptor,
  type ModelConfiguration,
} from "@/lib/languageModels/types";
import {
  updateReasoningEffortForChatSession,
  updateTemperatureOverrideForChatSession,
} from "@/app/app/services/lib";
import { useLLMProviders } from "@/lib/languageModels/hooks";

jest.mock("@/providers/UserProvider", () => ({
  useUser: () => ({ user: null }),
}));
jest.mock("@/lib/languageModels/hooks", () => ({
  useLLMProviders: jest.fn(),
}));
jest.mock("@/app/app/services/lib", () => ({
  updateReasoningEffortForChatSession: jest.fn(),
  updateTemperatureOverrideForChatSession: jest.fn(),
}));

const modelConfiguration = (
  name: string,
  id: number
): ModelConfiguration =>
  ({
    id,
    name,
    is_visible: true,
    max_input_tokens: null,
    supports_image_input: false,
    supports_reasoning: false,
    effectiveDisplayName: name,
  }) as ModelConfiguration;

const mockProviders = [
  {
    id: 1,
    name: "opencode",
    provider: "opencode",
    provider_display_name: "OpenCode Zen",
    model_configurations: [
      modelConfiguration("mimo-v2.5-free", 3),
      modelConfiguration("muse-spark-1.3-contributor-free", 5),
    ],
  },
] as LLMProviderDescriptor[];

function mockProviderList() {
  jest.mocked(useLLMProviders).mockReturnValue({
    llmProviders: mockProviders,
    defaultText: { provider_id: 1, model_name: "mimo-v2.5-free" },
    defaultVision: null,
    defaultChatNaming: null,
    defaultCraft: null,
    isLoading: false,
    error: undefined,
    refetch: jest.fn(),
  } as unknown as ReturnType<typeof useLLMProviders>);
}

function mockEmptyProviderList() {
  jest.mocked(useLLMProviders).mockReturnValue({
    llmProviders: [],
    defaultText: undefined,
    defaultVision: undefined,
    defaultChatNaming: undefined,
    defaultCraft: undefined,
    isLoading: false,
    error: undefined,
    refetch: jest.fn(),
  } as unknown as ReturnType<typeof useLLMProviders>);
}

function makeSession(
  id: string,
  reasoningEffort: ReasoningEffortOverride | null
): ChatSession {
  return {
    id,
    name: "",
    persona_id: 0,
    time_created: "",
    time_updated: "",
    shared_status: ChatSessionSharedStatus.Private,
    project_id: null,
    current_alternate_model: "",
    current_temperature_override: null,
    current_reasoning_effort_override: reasoningEffort,
  };
}

interface HookProps {
  session?: ChatSession;
}

describe("useLlmManager override persistence", () => {
  beforeEach(() => {
    mockEmptyProviderList();
    const ok = { ok: true, status: 200 } as Response;
    jest.mocked(updateReasoningEffortForChatSession).mockResolvedValue(ok);
    jest.mocked(updateTemperatureOverrideForChatSession).mockResolvedValue(ok);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  test("a persistOverrides reference taken before the selection writes the current choice", async () => {
    const { result } = renderHook(() => useLlmManager());
    // The send path can hold a reference from an earlier render.
    const persistOverrides = result.current.persistOverrides;

    act(() => result.current.updateReasoningEffort("high"));
    await act(async () => {
      await persistOverrides("session-1");
    });

    expect(updateReasoningEffortForChatSession).toHaveBeenCalledWith(
      "session-1",
      "high"
    );
  });

  test("a session persisted while unbound keeps the selection once it is adopted", async () => {
    const { result, rerender } = renderHook(
      (props: HookProps) => useLlmManager(props.session),
      { initialProps: {} }
    );

    act(() => result.current.updateReasoningEffort("high"));
    await act(async () => {
      await result.current.persistOverrides("session-1");
    });

    // The placeholder for the new session carries no override yet.
    rerender({ session: makeSession("session-1", null) });
    expect(result.current.reasoningEffort).toBe("high");

    // Another session still reads its own row.
    rerender({ session: makeSession("session-2", "low") });
    expect(result.current.reasoningEffort).toBe("low");

    // Coming back reads the row too, not the old local choice.
    rerender({ session: makeSession("session-1", null) });
    expect(result.current.reasoningEffort).toBeNull();
  });
});

describe("useLlmManager model handoff", () => {
  const spark = {
    name: "opencode",
    provider: "opencode",
    modelName: "muse-spark-1.3-contributor-free",
    modelConfigurationId: 5,
  };

  beforeEach(() => {
    mockProviderList();
  });

  test("arrival at a send-created session keeps the sent-with model", () => {
    const { result, rerender } = renderHook(
      (props: HookProps) => useLlmManager(props.session),
      { initialProps: {} }
    );

    act(() => result.current.updateCurrentLlm(spark));
    expect(result.current.currentLlm.modelName).toBe(spark.modelName);

    // Viewing session A, then new chat, then the send creates session B.
    rerender({ session: makeSession("session-a", null) });
    rerender({});
    act(() => result.current.noteModelHandoff("session-b", spark));
    rerender({ session: makeSession("session-b", null) });

    expect(result.current.currentLlm.modelName).toBe(spark.modelName);
  });

  test("arrival without a handoff still resets to the default model", () => {
    const { result, rerender } = renderHook(
      (props: HookProps) => useLlmManager(props.session),
      { initialProps: {} }
    );

    act(() => result.current.updateCurrentLlm(spark));
    rerender({ session: makeSession("session-a", null) });
    rerender({});
    rerender({ session: makeSession("session-b", null) });

    expect(result.current.currentLlm.modelName).toBe("mimo-v2.5-free");
  });

  test("a later navigation still resets after the handoff is consumed", () => {
    const { result, rerender } = renderHook(
      (props: HookProps) => useLlmManager(props.session),
      { initialProps: {} }
    );

    act(() => result.current.updateCurrentLlm(spark));
    act(() => result.current.noteModelHandoff("session-b", spark));
    rerender({ session: makeSession("session-b", null) });
    expect(result.current.currentLlm.modelName).toBe(spark.modelName);

    rerender({ session: makeSession("session-c", null) });
    expect(result.current.currentLlm.modelName).toBe("mimo-v2.5-free");
  });
});
