import { render, screen, waitFor } from "@tests/setup/test-utils";
import { useLLMProviders } from "@/lib/languageModels/hooks";
import { useSettings } from "@/lib/settings/hooks";
import LLMGatewayPage from "@/app/app/settings/llm-gateway/page";

const mockReplace = jest.fn();

jest.mock("next/navigation", () => ({
  useRouter: () => ({ replace: mockReplace }),
}));
jest.mock("@/lib/languageModels/hooks", () => ({
  useLLMProviders: jest.fn(),
}));
jest.mock("@/lib/settings/hooks", () => ({
  useSettings: jest.fn(),
}));
jest.mock("@/views/SettingsPage", () => ({
  LLMGatewaySettings: () => <div>Gateway settings</div>,
}));

const mockUseLLMProviders = useLLMProviders as jest.MockedFunction<
  typeof useLLMProviders
>;
const mockUseSettings = useSettings as jest.MockedFunction<typeof useSettings>;

describe("LLMGatewayPage", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockUseSettings.mockReturnValue({ isLoading: false } as ReturnType<
      typeof useSettings
    >);
    mockUseLLMProviders.mockReturnValue({
      llmProviders: [{ model_configurations: [{ is_visible: true }] }],
      isLoading: false,
      error: undefined,
    } as unknown as ReturnType<typeof useLLMProviders>);
  });

  // The gateway API ships outside this source tree, so the page must never
  // render: it redirects instead of calling endpoints that do not exist.
  it("redirects because the gateway is not part of this build", async () => {
    render(<LLMGatewayPage />);

    expect(screen.queryByText("Gateway settings")).not.toBeInTheDocument();
    await waitFor(() =>
      expect(mockReplace).toHaveBeenCalledWith("/app/settings/general")
    );
  });
});
