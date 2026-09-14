"use client";

import { useTranslations } from "next-intl";
import { useSWRConfig } from "swr";
import { useFormikContext } from "formik";
import { InputDivider, toast } from "@opal/layouts";
import {
  LLMProviderFormProps,
  LLMProviderName,
  LLMProviderView,
} from "@/lib/languageModels/types";
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
import type { KeyValue } from "@/refresh-components/inputs/InputKeyValue";

interface OpenAICompatibleModalValues extends BaseLLMFormValues {
  api_key: string;
  api_base: string;
}

interface OpenAICompatibleModalInternalsProps {
  existingLlmProvider: LLMProviderView | undefined;
  isOnboarding: boolean;
}

function OpenAICompatibleModalInternals({
  existingLlmProvider,
  isOnboarding,
}: OpenAICompatibleModalInternalsProps) {
  const t = useTranslations("admin.languageModels.modals");
  const formikProps = useFormikContext<OpenAICompatibleModalValues>();
  const apiBaseSubDescription = useApiBaseSubDescription(
    t("openAiCompatible.apiBaseField.description"),
    t("openAiCompatible.apiBaseField.learnMore")
  );

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
      <APIBaseField
        subDescription={apiBaseSubDescription}
        placeholder="http://localhost:8000/v1"
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

export interface OpenAICompatibleModalProps extends LLMProviderFormProps {
  /** Override the provider id (e.g. a dedicated gateway reusing this form). */
  providerOverride?: LLMProviderName;
  /** Extra headers prefilled for a brand-new provider (edits keep stored ones). */
  presetExtraHeaders?: KeyValue[];
}

export default function OpenAICompatibleModal({
  variant = "llm-configuration",
  existingLlmProvider,
  shouldMarkAsDefault,
  onOpenChange,
  onSuccess,
  analyticsSource,
  providerOverride,
  presetExtraHeaders,
}: OpenAICompatibleModalProps) {
  const t = useTranslations("admin.languageModels.modals");
  const isOnboarding = variant === "onboarding";
  const { mutate } = useSWRConfig();
  const providerName = providerOverride ?? LLMProviderName.OPENAI_COMPATIBLE;

  const onClose = () => onOpenChange?.(false);

  const initialValues = {
    ...useInitialValues(isOnboarding, providerName, existingLlmProvider),
    ...(!existingLlmProvider && presetExtraHeaders
      ? { extra_headers_list: presetExtraHeaders }
      : {}),
  } as OpenAICompatibleModalValues;

  const validationSchema = buildValidationSchema(t, isOnboarding, {
    apiBase: true,
  });

  return (
    <ModalWrapper
      providerName={providerName}
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
          providerName,
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
      <OpenAICompatibleModalInternals
        existingLlmProvider={existingLlmProvider}
        isOnboarding={isOnboarding}
      />
    </ModalWrapper>
  );
}
