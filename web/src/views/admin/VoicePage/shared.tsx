"use client";

import { markdown } from "@opal/utils";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";
import { Formik, Form, useFormikContext } from "formik";
import * as Yup from "yup";
import { SvgLumenLogo } from "@opal/logos";
import { Modal } from "@opal/components";
import { ConfirmationModalLayout } from "@opal/layouts";
import InputComboBoxField from "@/refresh-components/form/InputComboBoxField";
import InputTypeInField from "@/refresh-components/form/InputTypeInField";
import PasswordInputTypeInField from "@/refresh-components/form/PasswordInputTypeInField";
import InputSelectField from "@/refresh-components/form/InputSelectField";
import InputSelect from "@/refresh-components/inputs/InputSelect";
import { InputVertical, toast } from "@opal/layouts";
import { Section } from "@/layouts/general-layouts";
import { SvgArrowExchange, SvgUnplug, SvgSimpleLoader } from "@opal/icons";
import { Button, Text } from "@opal/components";
import { useModalClose } from "@opal/components";
import type {
  VoiceProviderView,
  VoiceFormValues,
  VoiceOption,
} from "@/lib/voice/types";
import {
  testVoiceProvider,
  upsertVoiceProvider,
  fetchVoicesByType,
  fetchVoiceModels,
  fetchAvailableVoices,
  clearVoiceProviderMode,
  deleteVoiceProvider,
  type FetchedVoice,
} from "@/lib/voice/svc";
import {
  getVoiceProviderDetail,
  resolveModelId,
  parseSttLanguages,
  sttLanguagesToInput,
  maxSttLanguagesForTargetUri,
  MAX_STT_LANGUAGES,
  STT_LOCALE_PATTERN,
  type ProviderMode,
} from "@/lib/voice/utils";

export { type ProviderMode } from "@/lib/voice/utils";

const AZURE_PORTAL_URL = "https://portal.azure.com/";

function mergeDiscoveredModels(
  base: Array<{ id: string; name: string }> | undefined,
  discovered: string[]
): Array<{ id: string; name: string }> {
  const seen = new Set((base ?? []).map((m) => m.id));
  const merged = [...(base ?? [])];
  for (const id of discovered) {
    if (!seen.has(id)) {
      seen.add(id);
      merged.push({ id, name: id });
    }
  }
  return merged;
}

// ---------------------------------------------------------------------------
// Fetches STT/TTS model ids from an OpenAI-compatible audio server
// (`GET {api_base}/models`) into the surrounding Formik-adjacent state.
// Shown only for the openai provider type, whose model fields are free-type.
// ---------------------------------------------------------------------------

function VoiceModelFetchButton({
  field,
  storedKeyRequest,
  onModels,
}: {
  /** Form field the fetched list fills in — the mode's model field. */
  field: "stt_model" | "tts_model";
  storedKeyRequest: () => { id?: number; use_stored_key?: boolean } | null;
  onModels: (ids: string[]) => void;
}) {
  const t = useTranslations("admin.voice");
  const { values, setFieldValue } = useFormikContext<VoiceFormValues>();
  const [isFetching, setIsFetching] = useState(false);

  const handleFetch = async () => {
    const apiBase = (values.api_base ?? "").trim();
    if (!apiBase) {
      toast.error(t("setupModal.fetchModels.missingBase"));
      return;
    }
    setIsFetching(true);
    // The form renders a masked key, never a usable one: ask the server to
    // read the stored key instead of echoing the mask back as a credential.
    const stored = storedKeyRequest();
    const { models, error } = await fetchVoiceModels({
      api_base: apiBase,
      ...(stored ?? { api_key: values.api_key || undefined }),
    });
    setIsFetching(false);
    if (error) {
      toast.error(error);
      return;
    }
    if (models.length === 0) {
      toast.error(t("setupModal.fetchModels.empty"));
      return;
    }
    onModels(models.map((m) => m.id));

    // Land on a real model: the server's own ids are the only ones that
    // work, so a default left over from the OpenAI catalogue never survives.
    const current = (values[field] ?? "").trim();
    const first = models[0]!.id;
    if (!current || !models.some((m) => m.id === current)) {
      void setFieldValue(field, first);
      toast.success(
        t("setupModal.fetchModels.selected", {
          count: models.length,
          model: first,
        })
      );
      return;
    }
    toast.success(
      t("setupModal.fetchModels.success", { count: models.length })
    );
  };

  return (
    <div className="flex flex-row justify-end w-full">
      <Button
        prominence="secondary"
        size="sm"
        disabled={isFetching}
        onClick={() => void handleFetch()}
      >
        {isFetching
          ? t("setupModal.fetchModels.loading")
          : t("setupModal.fetchModels.label")}
      </Button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Keeps the voice picker in step with the chosen TTS model. Self-hosted
// servers name their own speakers, so the list is read from the server
// (`GET {api_base}/audio/voices`) rather than from a static catalogue, and
// the first voice it returns becomes the default. Servers whose model ships
// no named voices answer with an empty list; the field stays free-typed.
// ---------------------------------------------------------------------------

const VOICE_REFETCH_DEBOUNCE_MS = 400;

function VoiceOptionsSync({
  providerType,
  isSelfHosted,
  storedKeyRequest,
  onVoices,
  onLoadingChange,
}: {
  providerType: string;
  isSelfHosted: boolean;
  storedKeyRequest: () => { id?: number; use_stored_key?: boolean } | null;
  onVoices: (voices: FetchedVoice[]) => void;
  onLoadingChange: (loading: boolean) => void;
}) {
  const { values, setFieldValue } = useFormikContext<VoiceFormValues>();
  const apiBase = (values.api_base ?? "").trim();
  const ttsModel = (values.tts_model ?? "").trim();
  // Read non-reactively: the key must not retrigger a fetch per keystroke,
  // and the current voice is only consulted once a list comes back.
  const apiKeyRef = useRef(values.api_key);
  const currentVoiceRef = useRef(values.default_voice);
  useEffect(() => {
    apiKeyRef.current = values.api_key;
    currentVoiceRef.current = values.default_voice;
  });

  const requestIdRef = useRef(0);
  // The model the form opened on. Switching away from it invalidates the
  // stored voice (speakers are per-model); reopening on it must not.
  const openedWithModelRef = useRef(ttsModel);

  useEffect(() => {
    const requestId = ++requestIdRef.current;

    const run = async () => {
      onLoadingChange(true);
      let voices: FetchedVoice[] = [];

      if (isSelfHosted) {
        if (apiBase) {
          const stored = storedKeyRequest();
          const result = await fetchAvailableVoices({
            provider_type: providerType,
            api_base: apiBase,
            tts_model: ttsModel || undefined,
            ...(stored ?? { api_key: apiKeyRef.current || undefined }),
          });
          voices = result.voices;
        }
      } else {
        const response = await fetchVoicesByType(providerType);
        voices = response.ok ? await response.json() : [];
      }

      if (requestId !== requestIdRef.current) return;
      onVoices(voices);
      onLoadingChange(false);

      // The first voice the server lists is the default; a stored voice
      // that the server doesn't know (e.g. an OpenAI name left on a custom
      // server) is replaced rather than kept and rejected at synthesis.
      const current = (currentVoiceRef.current ?? "").trim();
      if (voices.length > 0) {
        if (!voices.some((v) => v.id === current)) {
          void setFieldValue("default_voice", voices[0]!.id);
        }
      } else if (ttsModel !== openedWithModelRef.current && current) {
        // A model with no named voices: the speaker carried over from the
        // previous model no longer means anything, so start from empty.
        void setFieldValue("default_voice", "");
      }
    };

    const timer = setTimeout(() => void run(), VOICE_REFETCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerType, isSelfHosted, apiBase, ttsModel]);

  return null;
}

// ---------------------------------------------------------------------------
// VoiceProviderSetupModal
// ---------------------------------------------------------------------------

interface VoiceProviderSetupModalProps {
  providerType: string;
  existingProvider: VoiceProviderView | null;
  mode: ProviderMode;
  /**
   * True when this mode has no configuration yet, even if the provider row
   * already exists because the other mode was set up. Drives the header copy
   * and whether saving claims the default slot for this mode.
   */
  isNewForMode: boolean;
  defaultModelId?: string | null;
  onSuccess: () => void;
}

export function VoiceProviderSetupModal({
  providerType,
  existingProvider,
  mode,
  isNewForMode,
  defaultModelId,
  onSuccess,
}: VoiceProviderSetupModalProps) {
  const t = useTranslations("admin.voice");
  const onClose = useModalClose();
  const detail = getVoiceProviderDetail(providerType);
  // Custom servers speak the OpenAI wire protocol: same api_base field,
  // free-type model combos, and model Fetch as the openai provider type.
  const isOpenAIFamily =
    providerType === "openai" || providerType === "openai_compatible";
  // A self-hosted server names its own models and voices, so none of the
  // OpenAI catalogue defaults apply to it.
  const isSelfHosted = providerType === "openai_compatible";
  const initialTtsModel = defaultModelId
    ? resolveModelId(defaultModelId)
    : (existingProvider?.tts_model ?? (isSelfHosted ? "" : "tts-1"));

  // Non-form state: dynamic voice options
  const [voiceOptions, setVoiceOptions] = useState<VoiceOption[]>([]);
  const [isLoadingVoices, setIsLoadingVoices] = useState(mode === "tts");
  const [hasLoadedVoices, setHasLoadedVoices] = useState(false);

  const handleVoices = useCallback((voices: FetchedVoice[]) => {
    setVoiceOptions(
      voices.map((v) => ({ value: v.id, label: v.name, description: v.id }))
    );
    setHasLoadedVoices(true);
  }, []);

  // The stored key is shown masked, so it is never a usable credential:
  // discovery calls ask the server to read the saved one instead.
  const storedKeyRequest = useCallback(
    () =>
      existingProvider?.id && existingProvider.api_key
        ? { id: existingProvider.id, use_stored_key: true }
        : null,
    [existingProvider?.id, existingProvider?.api_key]
  );

  // Model ids discovered from an OpenAI-compatible audio server
  // (`GET {api_base}/models`), merged into the free-type STT/TTS combos.
  const [discoveredModels, setDiscoveredModels] = useState<string[]>([]);
  const handleDiscoveredModels = (ids: string[]) => {
    setDiscoveredModels((prev) => Array.from(new Set([...prev, ...ids])));
  };
  const sttModelOptions = mergeDiscoveredModels(
    detail.sttModels,
    discoveredModels
  );
  const ttsModelOptions = mergeDiscoveredModels(
    detail.ttsModels,
    discoveredModels
  );

  const modelRequiredMessage = t("setupModal.model.required");
  const validationSchema = Yup.object().shape({
    api_key: Yup.string().required(t("setupModal.apiKey.required")),
    target_uri:
      providerType === "azure"
        ? Yup.string().required(t("setupModal.targetUri.required"))
        : Yup.string(),
    // A custom server has no usable default model name, so one must be picked
    // rather than silently saved as an OpenAI id the server will reject.
    stt_model:
      isSelfHosted && mode === "stt"
        ? Yup.string().trim().required(modelRequiredMessage)
        : Yup.string(),
    tts_model:
      isSelfHosted && mode === "tts"
        ? Yup.string().trim().required(modelRequiredMessage)
        : Yup.string(),
    api_base: isSelfHosted
      ? Yup.string().trim().required(t("setupModal.apiBase.required"))
      : Yup.string(),
    // A custom server rejects a voice it doesn't know, and has no sane
    // fallback, so a speaker must be chosen or typed before saving.
    default_voice:
      isSelfHosted && mode === "tts"
        ? Yup.string().trim().required(t("setupModal.voice.required"))
        : Yup.string(),
    stt_languages:
      mode === "stt" && detail.sttLanguages
        ? Yup.string().test(
            "locales",
            t("setupModal.sttLanguages.invalid"),
            (value, context) => {
              if (!value) return true;
              const languages = parseSttLanguages(value);
              const bases = languages.map((lang) =>
                lang.split("-")[0]!.toLowerCase()
              );
              if (
                new Set(bases).size !== bases.length ||
                !languages.every((lang) => STT_LOCALE_PATTERN.test(lang))
              ) {
                return false;
              }
              const cap = maxSttLanguagesForTargetUri(
                context.parent.target_uri ?? ""
              );
              if (languages.length > cap) {
                return context.createError({
                  message:
                    cap === MAX_STT_LANGUAGES
                      ? t("setupModal.sttLanguages.azureCapExceeded", { cap })
                      : t("setupModal.sttLanguages.selfHostedCapExceeded", {
                          cap,
                        }),
                });
              }
              return true;
            }
          )
        : Yup.string(),
  });

  const initialValues: VoiceFormValues = {
    api_key: existingProvider?.api_key ?? "",
    target_uri: existingProvider?.target_uri ?? "",
    // The view maps api_base onto target_uri, so a stored self-hosted base
    // round-trips through it.
    api_base: isOpenAIFamily ? (existingProvider?.target_uri ?? "") : "",
    stt_model: existingProvider?.stt_model ?? (isSelfHosted ? "" : "whisper-1"),
    tts_model: initialTtsModel,
    // Voices are loaded, never assumed: VoiceOptionsSync fills this with the
    // first voice the provider actually offers.
    default_voice: existingProvider?.default_voice ?? "",
    stt_languages: sttLanguagesToInput(
      existingProvider?.custom_config?.stt_languages
    ),
  };

  async function handleSubmit(
    values: VoiceFormValues,
    { setSubmitting }: { setSubmitting: (v: boolean) => void }
  ) {
    const apiKeyChanged = values.api_key !== (existingProvider?.api_key ?? "");
    const shouldUseStoredKey = !apiKeyChanged && !!existingProvider?.api_key;

    try {
      if (!shouldUseStoredKey) {
        const testResponse = await testVoiceProvider({
          provider_type: providerType,
          api_key: apiKeyChanged ? values.api_key : undefined,
          target_uri:
            providerType === "azure"
              ? values.target_uri || undefined
              : undefined,
          api_base: isOpenAIFamily ? values.api_base || undefined : undefined,
          use_stored_key: shouldUseStoredKey,
        });

        if (!testResponse.ok) {
          const data = await testResponse.json().catch(() => ({}));
          toast.error(
            typeof data?.detail === "string"
              ? data.detail
              : t("setupModal.connectionTestFailed.message")
          );
          setSubmitting(false);
          return;
        }
      }

      // Preserve config keys the form doesn't own (e.g. speech_region).
      const customConfig: Record<string, unknown> = {
        ...existingProvider?.custom_config,
      };
      if (mode === "stt" && detail.sttLanguages) {
        const languages = parseSttLanguages(values.stt_languages);
        if (languages.length > 0) {
          customConfig.stt_languages = languages;
        } else {
          delete customConfig.stt_languages;
        }
      }

      // STT and TTS share one provider row, so each modal writes only the
      // fields it owns. Omitted fields are left untouched by the backend
      // instead of being overwritten with this form's placeholders.
      const response = await upsertVoiceProvider({
        id: existingProvider?.id,
        name: detail.label,
        provider_type: providerType,
        api_key: apiKeyChanged ? values.api_key : undefined,
        api_key_changed: apiKeyChanged,
        target_uri:
          providerType === "azure" ? values.target_uri || undefined : undefined,
        api_base: isOpenAIFamily ? values.api_base || undefined : undefined,
        custom_config: customConfig,
        stt_model:
          mode === "stt" ? values.stt_model.trim() || undefined : undefined,
        tts_model:
          mode === "tts" ? values.tts_model.trim() || undefined : undefined,
        default_voice:
          mode === "tts" ? values.default_voice.trim() || undefined : undefined,
        // Setting up a mode for the first time claims its default slot;
        // editing an already-configured one leaves the choice alone.
        activate_stt:
          mode === "stt"
            ? isNewForMode || (existingProvider?.is_default_stt ?? false)
            : (existingProvider?.is_default_stt ?? false),
        activate_tts:
          mode === "tts"
            ? isNewForMode || (existingProvider?.is_default_tts ?? false)
            : (existingProvider?.is_default_tts ?? false),
      });

      if (response.ok) {
        onSuccess();
      } else {
        const data = await response.json().catch(() => ({}));
        toast.error(
          typeof data?.detail === "string"
            ? data.detail
            : t("setupModal.saveError.message")
        );
      }
    } catch {
      toast.error(t("setupModal.saveError.message"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Modal open onOpenChange={onClose}>
      <Modal.Content width="sm">
        <Formik
          initialValues={initialValues}
          validationSchema={validationSchema}
          enableReinitialize
          onSubmit={handleSubmit}
        >
          {({ isSubmitting, dirty, isValid }) => (
            <Form>
              <Modal.Header
                icon={detail.icon}
                moreIcon1={SvgArrowExchange}
                moreIcon2={SvgLumenLogo}
                title={
                  isNewForMode
                    ? t("setupModal.createHeader.title", {
                        provider: detail.label,
                      })
                    : t("setupModal.editHeader.title", {
                        provider: detail.label,
                      })
                }
                description={t("setupModal.header.description", {
                  provider: detail.label,
                })}
                onClose={onClose}
              />
              <Modal.Body>
                {mode === "tts" && (
                  <VoiceOptionsSync
                    providerType={providerType}
                    isSelfHosted={isSelfHosted}
                    storedKeyRequest={storedKeyRequest}
                    onVoices={handleVoices}
                    onLoadingChange={setIsLoadingVoices}
                  />
                )}
                <Section gap={4} alignItems="stretch">
                  {providerType === "azure" && (
                    <InputVertical
                      title={t("setupModal.targetUri.label")}
                      subDescription={markdown(
                        t("setupModal.targetUri.description", {
                          portalUrl: AZURE_PORTAL_URL,
                        })
                      )}
                      withLabel="target_uri"
                    >
                      <InputTypeInField
                        name="target_uri"
                        placeholder="https://your_resource_region.tts.speech.microsoft.com/"
                      />
                    </InputVertical>
                  )}

                  {isOpenAIFamily && (
                    <InputVertical
                      title={t("setupModal.apiBase.label")}
                      subDescription={markdown(
                        t("setupModal.apiBase.description")
                      )}
                      withLabel="api_base"
                    >
                      <InputTypeInField
                        name="api_base"
                        placeholder="http://host.docker.internal:8080/v1"
                      />
                    </InputVertical>
                  )}

                  <InputVertical
                    title={t("setupModal.apiKey.label")}
                    subDescription={markdown(
                      t("setupModal.apiKey.description", {
                        url: detail.apiKeyUrl ?? "",
                        provider: detail.label,
                      })
                    )}
                    withLabel="api_key"
                  >
                    <PasswordInputTypeInField
                      name="api_key"
                      placeholder={t("setupModal.apiKey.placeholder")}
                    />
                  </InputVertical>

                  {mode === "stt" && detail.sttLanguages && (
                    <InputVertical
                      title={t("setupModal.sttLanguages.label")}
                      subDescription={markdown(
                        t("setupModal.sttLanguages.description", {
                          docsUrl: detail.sttLanguages.docsUrl,
                        })
                      )}
                      withLabel="stt_languages"
                    >
                      <InputTypeInField
                        name="stt_languages"
                        // oxlint-disable-next-line i18n/no-raw-jsx-text -- locale-code example, not copy
                        placeholder="en-US, fr-FR"
                      />
                    </InputVertical>
                  )}

                  {mode === "stt" &&
                    ((detail.sttModels?.length ?? 0) > 1 || isOpenAIFamily) && (
                      <InputVertical
                        title={t("setupModal.sttModel.label")}
                        subDescription={
                          isOpenAIFamily
                            ? t("setupModal.sttModel.openaiDescription")
                            : undefined
                        }
                        withLabel="stt_model"
                      >
                        {isOpenAIFamily ? (
                          <>
                            <InputComboBoxField
                              name="stt_model"
                              options={sttModelOptions.map((m) => ({
                                value: m.id,
                                label: m.name,
                              }))}
                              placeholder={t("setupModal.sttModel.placeholder")}
                              strict={false}
                            />
                            <VoiceModelFetchButton
                              field="stt_model"
                              storedKeyRequest={storedKeyRequest}
                              onModels={handleDiscoveredModels}
                            />
                          </>
                        ) : (
                          <InputSelectField name="stt_model">
                            <InputSelect.Trigger />
                            <InputSelect.Content>
                              {detail.sttModels!.map((m) => (
                                <InputSelect.Item key={m.id} value={m.id}>
                                  {m.name}
                                </InputSelect.Item>
                              ))}
                            </InputSelect.Content>
                          </InputSelectField>
                        )}
                      </InputVertical>
                    )}

                  {mode === "tts" && (
                    <>
                      {((detail.ttsModels?.length ?? 0) > 1 ||
                        isOpenAIFamily) && (
                        <InputVertical
                          title={t("setupModal.ttsModel.label")}
                          subDescription={
                            isOpenAIFamily
                              ? t("setupModal.ttsModel.openaiDescription")
                              : t("setupModal.ttsModel.description")
                          }
                          withLabel="tts_model"
                        >
                          {isOpenAIFamily ? (
                            <>
                              <InputComboBoxField
                                name="tts_model"
                                options={ttsModelOptions.map((m) => ({
                                  value: m.id,
                                  label: m.name,
                                }))}
                                placeholder={t(
                                  "setupModal.ttsModel.placeholder"
                                )}
                                strict={false}
                              />
                              <VoiceModelFetchButton
                                field="tts_model"
                                storedKeyRequest={storedKeyRequest}
                                onModels={handleDiscoveredModels}
                              />
                            </>
                          ) : (
                            <InputSelectField name="tts_model">
                              <InputSelect.Trigger />
                              <InputSelect.Content>
                                {detail.ttsModels!.map((m) => (
                                  <InputSelect.Item key={m.id} value={m.id}>
                                    {m.name}
                                  </InputSelect.Item>
                                ))}
                              </InputSelect.Content>
                            </InputSelectField>
                          )}
                        </InputVertical>
                      )}

                      <InputVertical
                        title={t("setupModal.voice.label")}
                        subDescription={
                          isSelfHosted
                            ? markdown(
                                hasLoadedVoices && voiceOptions.length === 0
                                  ? t("setupModal.voice.customEmptyDescription")
                                  : t("setupModal.voice.customDescription")
                              )
                            : markdown(
                                t("setupModal.voice.description", {
                                  docsLabel:
                                    detail.voiceDocsUrl?.label ?? detail.label,
                                  docsUrl:
                                    detail.voiceDocsUrl?.url ??
                                    detail.docsUrl ??
                                    "",
                                })
                              )
                        }
                        withLabel="default_voice"
                      >
                        {voiceOptions.length > 0 ? (
                          <InputComboBoxField
                            name="default_voice"
                            options={voiceOptions}
                            placeholder={
                              isLoadingVoices
                                ? t("setupModal.voice.loadingPlaceholder")
                                : t("setupModal.voice.placeholder")
                            }
                            disabled={isLoadingVoices}
                            strict={false}
                          />
                        ) : (
                          // The combo box commits free text only by picking it
                          // out of its dropdown, which has nothing to show when
                          // the server lists no voices. A plain field keeps the
                          // speaker name typeable.
                          <InputTypeInField
                            name="default_voice"
                            placeholder={
                              isLoadingVoices
                                ? t("setupModal.voice.loadingPlaceholder")
                                : t("setupModal.voice.freeTextPlaceholder")
                            }
                          />
                        )}
                      </InputVertical>
                    </>
                  )}
                </Section>
              </Modal.Body>
              <Modal.Footer>
                <Button prominence="secondary" onClick={onClose}>
                  {t("setupModal.cancelButton.label")}
                </Button>
                <Button
                  type="submit"
                  disabled={isSubmitting || !isValid || !dirty}
                  icon={isSubmitting ? SvgSimpleLoader : undefined}
                >
                  {isNewForMode
                    ? t("setupModal.connectButton.label")
                    : t("setupModal.updateButton.label")}
                </Button>
              </Modal.Footer>
            </Form>
          )}
        </Formik>
      </Modal.Content>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// VoiceDisconnectModal
// ---------------------------------------------------------------------------

interface VoiceDisconnectModalProps {
  disconnectTarget: {
    providerId: number;
    providerLabel: string;
    providerType: string;
  };
  /**
   * Mode being disconnected. A self-hosted row backs both STT and TTS, so
   * only that mode's configuration is cleared; the row goes when nothing is
   * left on it.
   */
  mode: ProviderMode;
  perModeDisconnect: boolean;
  hasAlternatives: boolean;
  onSuccess: () => void;
}

export function VoiceDisconnectModal({
  disconnectTarget,
  mode,
  perModeDisconnect,
  hasAlternatives,
  onSuccess,
}: VoiceDisconnectModalProps) {
  const t = useTranslations("admin.voice");
  const onClose = useModalClose();
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function handleDisconnect() {
    setIsSubmitting(true);
    try {
      const res = perModeDisconnect
        ? await clearVoiceProviderMode(disconnectTarget.providerId, mode)
        : await deleteVoiceProvider(disconnectTarget.providerId);
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(
          typeof body?.detail === "string"
            ? body.detail
            : t("disconnectModal.disconnectError.message")
        );
      }
      toast.success(
        t("disconnectModal.disconnectSuccess.message", {
          label: disconnectTarget.providerLabel,
        })
      );
      onSuccess();
      onClose?.();
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : t("unexpectedError.message")
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <ConfirmationModalLayout
      icon={SvgUnplug}
      title={t("disconnectModal.header.title", {
        label: disconnectTarget.providerLabel,
      })}
      description={
        perModeDisconnect
          ? t("disconnectModal.header.modeDescription")
          : t("disconnectModal.header.description")
      }
      submit={
        <Button
          variant="danger"
          onClick={() => void handleDisconnect()}
          disabled={isSubmitting}
        >
          {t("disconnectModal.submitButton.label")}
        </Button>
      }
    >
      <Section alignItems="start" gap={2}>
        <Text color="text-03">
          {markdown(
            perModeDisconnect
              ? t(
                  mode === "stt"
                    ? "disconnectModal.body.sttDescription"
                    : "disconnectModal.body.ttsDescription",
                  { label: disconnectTarget.providerLabel }
                )
              : t("disconnectModal.body.description", {
                  label: disconnectTarget.providerLabel,
                })
          )}
        </Text>
        {!hasAlternatives && (
          <Text color="text-03">
            {t("disconnectModal.connectAnother.description")}
          </Text>
        )}
      </Section>
    </ConfirmationModalLayout>
  );
}
