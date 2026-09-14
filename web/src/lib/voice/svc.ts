const VOICE_PROVIDERS_URL = "/api/admin/voice/providers";

/** Sets a provider as the active STT or TTS default. Optionally pins a specific TTS model. */
export async function activateVoiceProvider(
  providerId: number,
  mode: "stt" | "tts",
  ttsModel?: string
): Promise<Response> {
  const url = new URL(
    `${VOICE_PROVIDERS_URL}/${providerId}/activate-${mode}`,
    window.location.origin
  );
  if (mode === "tts" && ttsModel) {
    url.searchParams.set("tts_model", ttsModel);
  }
  return fetch(url.toString(), { method: "POST" });
}

/** Removes the STT or TTS default status from a provider without deleting it. */
export async function deactivateVoiceProvider(
  providerId: number,
  mode: "stt" | "tts"
): Promise<Response> {
  return fetch(`${VOICE_PROVIDERS_URL}/${providerId}/deactivate-${mode}`, {
    method: "POST",
  });
}

/** Validates provider credentials with a live API call before saving. */
export async function testVoiceProvider(request: {
  provider_type: string;
  api_key?: string;
  target_uri?: string;
  api_base?: string;
  use_stored_key?: boolean;
}): Promise<Response> {
  return fetch(`${VOICE_PROVIDERS_URL}/test`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

/** Creates or updates a voice provider configuration. */
export async function upsertVoiceProvider(
  request: Record<string, unknown>
): Promise<Response> {
  return fetch(VOICE_PROVIDERS_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
}

/** Fetches the list of available voices for a given provider type. */
export async function fetchVoicesByType(
  providerType: string
): Promise<Response> {
  return fetch(`/api/admin/voice/voices?provider_type=${providerType}`);
}

export interface FetchedVoice {
  id: string;
  name: string;
}

/**
 * Lists the voices a provider offers for a given TTS model. Self-hosted
 * servers are asked directly (`GET {base}/audio/voices`); an empty list is a
 * valid answer for models that ship no named voices.
 */
export async function fetchAvailableVoices(request: {
  provider_type: string;
  api_base?: string;
  api_key?: string;
  tts_model?: string;
  id?: number;
  use_stored_key?: boolean;
}): Promise<{ voices: FetchedVoice[]; error?: string }> {
  try {
    const response = await fetch("/api/admin/voice/available-voices", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      return {
        voices: [],
        error:
          errorData.detail || errorData.message || "Failed to fetch voices",
      };
    }
    return { voices: await response.json() };
  } catch (error) {
    return {
      voices: [],
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}

export interface FetchedVoiceModel {
  id: string;
}

/**
 * Lists models on an OpenAI-compatible audio server (`GET {base}/models`).
 * Embeddings are excluded server-side; pick an STT/TTS model, then the
 * connection test verifies it transcribes/synthesizes.
 */
export async function fetchVoiceModels(request: {
  api_base: string;
  api_key?: string;
  id?: number;
  use_stored_key?: boolean;
}): Promise<{ models: FetchedVoiceModel[]; error?: string }> {
  try {
    const response = await fetch("/api/admin/voice/available-models", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    if (!response.ok) {
      let errorMessage = "Failed to fetch models";
      try {
        const errorData = await response.json();
        errorMessage = errorData.detail || errorData.message || errorMessage;
      } catch {
        // ignore JSON parsing errors
      }
      return { models: [], error: errorMessage };
    }
    const models: FetchedVoiceModel[] = await response.json();
    return { models };
  } catch (error) {
    return {
      models: [],
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}

/**
 * Disconnects one mode from a provider. STT and TTS share a provider row, so
 * the row survives as long as the other mode is still configured.
 */
export async function clearVoiceProviderMode(
  providerId: number,
  mode: "stt" | "tts"
): Promise<Response> {
  return fetch(`${VOICE_PROVIDERS_URL}/${providerId}/clear/${mode}`, {
    method: "POST",
  });
}

/** Permanently removes a voice provider and its stored credentials. */
export async function deleteVoiceProvider(
  providerId: number
): Promise<Response> {
  return fetch(`${VOICE_PROVIDERS_URL}/${providerId}`, { method: "DELETE" });
}

/** Fetches all configured LLM providers (used to copy API keys into voice providers). */
export async function fetchLLMProviders(): Promise<Response> {
  return fetch("/api/admin/llm/provider");
}
