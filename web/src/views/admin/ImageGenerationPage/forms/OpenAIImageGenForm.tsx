"use client";

import React, { useMemo, useState } from "react";
import { useTranslations } from "next-intl";
import { Button } from "@opal/components";
import { toast } from "@opal/layouts";
import { InputTypeIn, PasswordInputTypeIn } from "@opal/components";
import * as Yup from "yup";
import { FormikField } from "@/refresh-components/form/FormikField";
import { FormField } from "@/refresh-components/form/FormField";
import InputComboBox from "@/refresh-components/inputs/InputComboBox";
import { ImageGenFormWrapper } from "@/views/admin/ImageGenerationPage/forms/ImageGenFormWrapper";
import {
  ImageGenFormBaseProps,
  ImageGenFormChildProps,
  ImageGenSubmitPayload,
} from "@/views/admin/ImageGenerationPage/forms/types";
import {
  ImageGenerationCredentials,
  fetchImageGenModels,
} from "@/views/admin/ImageGenerationPage/svc";
import { ImageProvider } from "@/views/admin/ImageGenerationPage/constants";

// OpenAI form values - API key plus optional self-hosted overrides
interface OpenAIFormValues {
  api_key: string;
  /** OpenAI-compatible base URL (e.g. LocalAI). Empty = api.openai.com. */
  api_base: string;
  /** Model name override (e.g. a LocalAI gallery model). Empty = catalog model. */
  model_name: string;
}

const initialValues: OpenAIFormValues = {
  api_key: "",
  api_base: "",
  model_name: "",
};

function OpenAIFormFields(props: ImageGenFormChildProps<OpenAIFormValues>) {
  const t = useTranslations("admin.imageGeneration");
  const {
    apiStatus,
    showApiMessage,
    errorMessage,
    disabled,
    isLoadingCredentials,
    apiKeyOptions,
    resetApiState,
    imageProvider,
    formikProps,
  } = props;

  const [modelOptions, setModelOptions] = useState<
    { value: string; label: string }[]
  >([]);
  const [isFetchingModels, setIsFetchingModels] = useState(false);

  const handleFetchModels = async () => {
    const apiBase = formikProps.values.api_base.trim();
    if (!apiBase) {
      toast.error(t("form.fetchModels.missingBase"));
      return;
    }
    setIsFetchingModels(true);
    const { models, error } = await fetchImageGenModels(
      apiBase,
      formikProps.values.api_key || undefined
    );
    setIsFetchingModels(false);
    if (error) {
      toast.error(error);
      return;
    }
    if (models.length === 0) {
      toast.error(t("form.fetchModels.empty"));
      return;
    }
    setModelOptions(models.map((m) => ({ value: m.name, label: m.name })));
    toast.success(t("form.fetchModels.success", { count: models.length }));
  };

  return (
    <>
      <FormikField<string>
        name="api_base"
        render={(field, helper, meta, state) => (
          <FormField
            name="api_base"
            state={apiStatus === "error" ? "error" : state}
            className="w-full"
          >
            <FormField.Label>{t("form.apiBase.label")}</FormField.Label>
            <FormField.Control>
              <InputTypeIn
                {...field}
                onChange={(e) => {
                  field.onChange(e);
                  resetApiState();
                }}
                placeholder="http://host.docker.internal:8080/v1"
                variant={disabled ? "disabled" : undefined}
              />
            </FormField.Control>
            <FormField.Message
              messages={{
                idle: t("form.apiBase.idle"),
                error: meta.error,
              }}
            />
          </FormField>
        )}
      />
      <FormikField<string>
        name="model_name"
        render={(field, helper, meta, state) => (
          <FormField
            name="model_name"
            state={apiStatus === "error" ? "error" : state}
            className="w-full"
          >
            <FormField.Label>{t("form.modelName.label")}</FormField.Label>
            <FormField.Control>
              <div className="flex flex-row items-center gap-2 w-full">
                <div className="grow min-w-0">
                  <InputComboBox
                    value={field.value ?? ""}
                    onChange={(e) => {
                      helper.setValue(e.target.value);
                      resetApiState();
                    }}
                    onValueChange={(value) => {
                      helper.setValue(value);
                      resetApiState();
                    }}
                    onBlur={field.onBlur}
                    options={modelOptions}
                    placeholder={
                      imageProvider.model_name ||
                      t("form.modelName.customPlaceholder")
                    }
                    disabled={disabled}
                    isError={apiStatus === "error"}
                  />
                </div>
                <Button
                  prominence="secondary"
                  size="sm"
                  disabled={disabled || isFetchingModels}
                  onClick={() => void handleFetchModels()}
                >
                  {isFetchingModels
                    ? t("form.fetchModels.loading")
                    : t("form.fetchModels.label")}
                </Button>
              </div>
            </FormField.Control>
            <FormField.Message
              messages={{
                idle: t("form.modelName.idle", {
                  model: imageProvider.model_name,
                }),
                error: meta.error,
              }}
            />
          </FormField>
        )}
      />
      <FormikField<string>
        name="api_key"
        render={(field, helper, meta, state) => (
          <FormField
            name="api_key"
            state={apiStatus === "error" ? "error" : state}
            className="w-full"
          >
            <FormField.Label>{t("form.apiKey.label")}</FormField.Label>
            <FormField.Control>
              {apiKeyOptions.length > 0 ? (
                <InputComboBox
                  value={field.value}
                  onChange={(e) => {
                    helper.setValue(e.target.value);
                    resetApiState();
                  }}
                  onValueChange={(value) => {
                    helper.setValue(value);
                    resetApiState();
                  }}
                  onBlur={field.onBlur}
                  options={apiKeyOptions}
                  placeholder={
                    isLoadingCredentials
                      ? t("form.loading.placeholder")
                      : t("form.apiKey.comboPlaceholder")
                  }
                  disabled={disabled}
                  isError={apiStatus === "error"}
                />
              ) : (
                <PasswordInputTypeIn
                  {...field}
                  onChange={(e) => {
                    field.onChange(e);
                    resetApiState();
                  }}
                  placeholder={
                    isLoadingCredentials
                      ? t("form.loading.placeholder")
                      : t("form.apiKey.placeholder")
                  }
                  disabled={disabled}
                  error={apiStatus === "error"}
                />
              )}
            </FormField.Control>
            {showApiMessage ? (
              <FormField.APIMessage
                state={apiStatus}
                messages={{
                  loading: t("form.apiKeyTest.loading", {
                    title: imageProvider.title,
                  }),
                  success: t("form.apiKeyTest.success"),
                  error: errorMessage || t("form.apiKeyTest.error"),
                }}
              />
            ) : (
              <FormField.Message
                messages={{
                  idle: t("form.apiKey.idle"),
                  error: meta.error,
                }}
              />
            )}
          </FormField>
        )}
      />
    </>
  );
}

function getInitialValuesFromCredentials(
  credentials: ImageGenerationCredentials,
  _imageProvider: ImageProvider,
  storedModelName?: string
): Partial<OpenAIFormValues> {
  return {
    api_key: credentials.api_key || "",
    api_base: credentials.api_base || "",
    // Show the stored custom model on edit (empty = catalog model).
    model_name:
      storedModelName && storedModelName !== _imageProvider.model_name
        ? storedModelName
        : "",
  };
}

function transformValues(
  values: OpenAIFormValues,
  imageProvider: ImageProvider,
  storedModelName?: string
): ImageGenSubmitPayload {
  // Empty override keeps a previously stored custom model on edit.
  const storedOverride =
    storedModelName && storedModelName !== imageProvider.model_name
      ? storedModelName
      : undefined;
  return {
    modelName:
      values.model_name.trim() || storedOverride || imageProvider.model_name,
    imageProviderId: imageProvider.image_provider_id,
    provider: "openai",
    apiKey: values.api_key,
    apiBase: values.api_base.trim() || undefined,
  };
}

export function OpenAIImageGenForm(props: ImageGenFormBaseProps) {
  const t = useTranslations("admin.imageGeneration");
  const { imageProvider, existingConfig } = props;

  const validationSchema = useMemo(
    () =>
      Yup.object().shape({
        api_key: Yup.string().required(t("form.apiKey.required")),
        // Custom cards have no catalog model: a model is mandatory.
        ...(imageProvider.model_name
          ? {}
          : { model_name: Yup.string().required(t("form.modelName.required")) }),
      }),
    [t, imageProvider.model_name]
  );

  return (
    <ImageGenFormWrapper<OpenAIFormValues>
      {...props}
      title={
        existingConfig
          ? t("form.editHeader.title", { title: imageProvider.title })
          : t("form.connectHeader.title", { title: imageProvider.title })
      }
      description={t(imageProvider.descriptionKey)}
      initialValues={initialValues}
      validationSchema={validationSchema}
      getInitialValuesFromCredentials={(creds) =>
        getInitialValuesFromCredentials(
          creds,
          imageProvider,
          existingConfig?.model_name
        )
      }
      transformValues={(values) =>
        transformValues(values, imageProvider, existingConfig?.model_name)
      }
    >
      {(childProps) => <OpenAIFormFields {...childProps} />}
    </ImageGenFormWrapper>
  );
}
