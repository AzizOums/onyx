"use client";

import { useTranslations } from "next-intl";
import { useSWRConfig } from "swr";
import { useFormikContext } from "formik";
import { InputDivider, toast } from "@opal/layouts";
import { Tabs, Text } from "@opal/components";
import {
  LLMProviderFormProps,
  LLMProviderName,
  LLMProviderView,
  OpencodeApiMode,
} from "@/lib/languageModels/types";
import type { KeyValue } from "@/refresh-components/inputs/InputKeyValue";
import { fetchOpenAICompatibleModels } from "@/lib/languageModels/svc";
import {
  useInitialValues,
  buildValidationSchema,
  BaseLLMFormValues,
  withFetchedModels,
} from "@/sections/modals/languageModels/utils";
import { submitProvider } from "@/sections/modals/languageModels/svc";
import { LLMProviderConfiguredSource } from "@/lib/analytics/utils";
import {
  APIBaseField,
  APIKeyField,
  ModelSelectionField,
  DisplayNameField,
  ExtraHeadersField,
  ModelAccessField,
  ModalWrapper,
  useApiBaseSubDescription,
} from "@/sections/modals/languageModels/shared";
import { refreshLlmProviderCaches } from "@/lib/languageModels/cache";
import { ModelsDevBrowser } from "@/sections/modals/languageModels/ModelsDevBrowser";

// Static client identity for the Zen gateway, prefilled so a new provider
// works out of the box (free tier needs no API key). Keep in sync with
// backend/onyx/llm/opencode.py. Session/request ids are minted per chat
// session and turn by the backend — never store them here.
const PRESET_EXTRA_HEADERS: KeyValue[] = [
  { key: "User-Agent", value: "opencode/1.18.18" },
  { key: "x-opencode-client", value: "cli" },
];

const DEFAULT_API_MODE: OpencodeApiMode = "chat_completions";
const OPENCODE_API_MODE_KEY = "opencode_api_mode";

const API_MODE_TABS = [
  {
    value: "chat_completions",
    titleKey: "bifrost.apiMode.chatCompletions.label",
    subtitle: "/v1/chat/completions",
  },
  {
    value: "responses",
    titleKey: "bifrost.apiMode.responses.label",
    subtitle: "/v1/responses",
  },
] as const satisfies readonly {
  value: OpencodeApiMode;
  titleKey: string;
  subtitle: string;
}[];

interface OpencodeModalValues extends BaseLLMFormValues {
  api_key: string;
  api_base: string;
}

interface OpencodeModalInternalsProps {
  existingLlmProvider: LLMProviderView | undefined;
  isOnboarding: boolean;
}

function OpencodeModalInternals({
  existingLlmProvider,
  isOnboarding,
}: OpencodeModalInternalsProps) {
  const t = useTranslations("admin.languageModels.modals");
  const formikProps = useFormikContext<OpencodeModalValues>();
  const { setFieldValue, values } = formikProps;
  const apiBaseSubDescription = useApiBaseSubDescription(
    t("openAiCompatible.apiBaseField.description"),
    t("openAiCompatible.apiBaseField.learnMore")
  );

  const mode =
    (values.custom_config?.[OPENCODE_API_MODE_KEY] as
      | OpencodeApiMode
      | undefined) ?? DEFAULT_API_MODE;

  const isFetchDisabled = !formikProps.values.api_base;

  const handleFetchModels = async () => {
    const { models, error } = await fetchOpenAICompatibleModels({
      api_base: formikProps.values.api_base,
      api_key: formikProps.values.api_key || undefined,
      provider_id: existingLlmProvider?.id ?? undefined,
    });
    if (error) {
      throw new Error(error);
    }
    formikProps.setValues(withFetchedModels(models));
  };

  return (
    <>
      <Tabs
        value={mode}
        onValueChange={(next) =>
          setFieldValue("custom_config", {
            ...values.custom_config,
            [OPENCODE_API_MODE_KEY]: next as OpencodeApiMode,
          })
        }
      >
        <Tabs.List>
          {API_MODE_TABS.map((tab) => (
            <Tabs.Trigger key={tab.value} value={tab.value}>
              <div className="flex flex-col items-start">
                <Text font="main-ui-action" color="inherit">
                  {t(tab.titleKey)}
                </Text>
                <Text font="secondary-body" color="text-03">
                  {tab.subtitle}
                </Text>
              </div>
            </Tabs.Trigger>
          ))}
        </Tabs.List>
      </Tabs>

      <APIBaseField
        subDescription={apiBaseSubDescription}
        placeholder="https://opencode.ai/zen/v1"
      />

      <ModelsDevBrowser
        onApply={(picked) =>
          formikProps.setValues((prev) => ({
            ...prev,
            model_configurations: picked.map((m) => ({
              name: m.id,
              display_name: m.name,
              is_visible: true,
              max_input_tokens: m.context_limit,
              supports_image_input: m.supports_image_input,
              supports_audio_input: m.supports_audio_input,
              supports_video_input: m.supports_video_input,
              supports_reasoning: m.reasoning,
              effectiveDisplayName: m.name,
            })),
          }))
        }
      />

      <APIKeyField
        optional
        subDescription={t("openAiCompatible.apiKeyField.description")}
      />

      {!isOnboarding && (
        <>
          <InputDivider />
          <ExtraHeadersField />
          <InputDivider />
          <DisplayNameField />
        </>
      )}

      <InputDivider />
      <ModelSelectionField
        shouldShowAutoUpdateToggle={false}
        onRefetch={isFetchDisabled ? undefined : handleFetchModels}
      />

      {!isOnboarding && (
        <>
          <InputDivider />
          <ModelAccessField />
        </>
      )}
    </>
  );
}

export default function OpencodeModal({
  variant = "llm-configuration",
  existingLlmProvider,
  shouldMarkAsDefault,
  onOpenChange,
  onSuccess,
  analyticsSource,
}: LLMProviderFormProps) {
  const t = useTranslations("admin.languageModels.modals");
  const isOnboarding = variant === "onboarding";
  const { mutate } = useSWRConfig();

  const onClose = () => onOpenChange?.(false);

  const initialValues = {
    ...useInitialValues(isOnboarding, LLMProviderName.OPENCODE, existingLlmProvider),
    ...(!existingLlmProvider
      ? { extra_headers_list: PRESET_EXTRA_HEADERS }
      : {}),
  } as OpencodeModalValues;

  // useInitialValues drops custom_config, so seed it for submitProvider's
  // custom_config_changed diff, preserving any other stored entries.
  // Pre-existing providers without a stored mode keep old chat behavior.
  const initialMode =
    (existingLlmProvider?.custom_config?.[OPENCODE_API_MODE_KEY] as
      | OpencodeApiMode
      | undefined) ?? DEFAULT_API_MODE;
  initialValues.custom_config = {
    ...existingLlmProvider?.custom_config,
    [OPENCODE_API_MODE_KEY]: initialMode,
  };

  const validationSchema = buildValidationSchema(t, isOnboarding, {
    apiBase: true,
  });

  return (
    <ModalWrapper
      providerName={LLMProviderName.OPENCODE}
      llmProvider={existingLlmProvider}
      onClose={onClose}
      initialValues={initialValues}
      description={t("openAiCompatible.description")}
      validationSchema={validationSchema}
      onSubmit={async (values, { setSubmitting, setStatus }) => {
        await submitProvider({
          t,
          analyticsSource:
            analyticsSource ??
            (isOnboarding
              ? LLMProviderConfiguredSource.CHAT_ONBOARDING
              : LLMProviderConfiguredSource.ADMIN_PAGE),
          providerName: LLMProviderName.OPENCODE,
          values,
          initialValues,
          existingLlmProvider,
          shouldMarkAsDefault,
          setStatus,
          setSubmitting,
          onClose,
          onSuccess: async () => {
            if (onSuccess) {
              await onSuccess();
            } else {
              await refreshLlmProviderCaches(mutate);
              toast.success(
                existingLlmProvider
                  ? t("toasts.providerUpdated")
                  : t("toasts.providerEnabled")
              );
            }
          },
        });
      }}
    >
      <OpencodeModalInternals
        existingLlmProvider={existingLlmProvider}
        isOnboarding={isOnboarding}
      />
    </ModalWrapper>
  );
}
