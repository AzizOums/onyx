"use client";

import { useState } from "react";
import { Button, InputTypeIn, Text } from "@opal/components";
import { Section, toast } from "@opal/layouts";
import { SvgSearch } from "@opal/icons";
import { SvgSimpleLoader } from "@opal/icons";
import {
  fetchModelsDevProviderModels,
  fetchModelsDevProviders,
} from "@/lib/languageModels/svc";
import type {
  ModelsDevModelInfo,
  ModelsDevProviderInfo,
} from "@/lib/languageModels/types";


function modalitySummary(model: ModelsDevModelInfo): string {
  const parts: string[] = [];
  if (model.supports_image_input) parts.push("Image");
  if (model.supports_audio_input) parts.push("Audio");
  if (model.supports_video_input) parts.push("Video");
  if (model.supports_pdf_input) parts.push("PDF");
  if (parts.length === 0) parts.push("Text");
  if (model.context_limit) {
    parts.push(`${(model.context_limit / 1000).toFixed(0)}K ctx`);
  }
  return parts.join(" · ");
}

/**
 * models.dev catalog browser for the admin LLM-provider flows.
 *
 * Lets the admin pick a provider from the public models.dev catalog:
 * selecting one shows the API base, auth env keys, docs link, and the
 * model list with input modalities. Selected models are merged back into
 * the form's model_configurations with image/audio/video capability flags
 * set from the catalog.
 */
export function ModelsDevBrowser({
  onApply,
}: {
  onApply: (models: ModelsDevModelInfo[]) => void;
}) {
  const [query, setQuery] = useState("");
  const [providers, setProviders] = useState<ModelsDevProviderInfo[] | null>(
    null
  );
  const [providersLoading, setProvidersLoading] = useState(false);
  const [selectedProvider, setSelectedProvider] =
    useState<ModelsDevProviderInfo | null>(null);
  const [models, setModels] = useState<ModelsDevModelInfo[] | null>(null);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const loadProviders = async () => {
    setProvidersLoading(true);
    try {
      const list = await fetchModelsDevProviders();
      setProviders(list);
    } catch (e) {
      toast.error(
        `Failed to load models.dev providers: ${
          e instanceof Error ? e.message : "unknown"
        }`
      );
    } finally {
      setProvidersLoading(false);
    }
  };

  const loadModels = async (provider: ModelsDevProviderInfo) => {
    setSelectedProvider(provider);
    setModelsLoading(true);
    setSelectedIds(new Set());
    try {
      const list = await fetchModelsDevProviderModels(provider.id);
      setModels(list);
    } catch (e) {
      toast.error(
        `Failed to load models: ${e instanceof Error ? e.message : "unknown"}`
      );
    } finally {
      setModelsLoading(false);
    }
  };

  const applySelection = () => {
    if (!models) return;
    onApply(models.filter((m) => selectedIds.has(m.id)));
  };

  const filteredProviders = (providers ?? []).filter((p) =>
    p.name.toLowerCase().includes(query.toLowerCase())
  );

  if (providers === null) {
    return (
      <Section justifyContent="center" padding={2}>
        <Button
          icon={SvgSearch}
          onClick={loadProviders}
          prominence="secondary"
          disabled={providersLoading}
        >
          {providersLoading ? "Loading catalog…" : "Browse models.dev catalog"}
        </Button>
      </Section>
    );
  }

  return (
    <Section gap={1}>
      <Section
        flexDirection="row"
        justifyContent="between"
        alignItems="center"
        gap={2}
      >
        <Text font="secondary-action">
          {selectedProvider?.name ?? "models.dev catalog"}
        </Text>
        {selectedProvider ? (
          <Button
            prominence="tertiary"
            onClick={() => {
              setSelectedProvider(null);
              setModels(null);
            }}
          >
            Back
          </Button>
        ) : (
          <InputTypeIn
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search providers…"
            searchIcon
            data-testid="modelsdev-search"
          />
        )}
      </Section>

      {selectedProvider && (
        <Section gap={0.25} flexDirection="column">
          <Text font="secondary-body" color="text-03">
            {`API: ${selectedProvider.api ?? "n/a"} — auth env keys: ${
              selectedProvider.env_keys.join(", ") || "none"
            }`}
          </Text>
          {selectedProvider.doc && (
            <a href={selectedProvider.doc} target="_blank" rel="noreferrer">
              <Text font="secondary-body" color="text-03">
                Documentation
              </Text>
            </a>
          )}
        </Section>
      )}

      {providersLoading || modelsLoading ? (
        <SvgSimpleLoader />
      ) : selectedProvider ? (
        <Section gap={0.5}>
          {(models ?? []).map((model) => (
            <Section
              key={model.id}
              flexDirection="row"
              justifyContent="between"
              alignItems="center"
              gap={2}
            >
              <Section gap={0.5} flexDirection="column">
                <Text font="main-ui-body">{model.name}</Text>
                <Text font="secondary-body" color="text-03">
                  {modalitySummary(model)}
                </Text>
              </Section>
              <Button
                type="button"
                prominence={selectedIds.has(model.id) ? "primary" : "secondary"}
                data-testid={`modelsdev-add-${model.id}`}
                onClick={() => {
                  const next = new Set(selectedIds);
                  if (next.has(model.id)) {
                    next.delete(model.id);
                  } else {
                    next.add(model.id);
                  }
                  setSelectedIds(next);
                }}
              >
                {selectedIds.has(model.id) ? "Selected" : "Add"}
              </Button>
            </Section>
          ))}
          <Section padding={1} justifyContent="end" flexDirection="row">
              <Button
                type="button"
                prominence="primary"
                data-testid="modelsdev-apply"
                disabled={selectedIds.size === 0}
                onClick={applySelection}
              >
              {`Add ${selectedIds.size} model(s) to the provider`}
            </Button>
          </Section>
        </Section>
      ) : (
        <Section gap={0.5}>
          {filteredProviders.slice(0, 40).map((provider) => (
            <Section
              key={provider.id}
              flexDirection="row"
              justifyContent="between"
              alignItems="center"
              gap={2}
            >
              <Section gap={0.5} flexDirection="column">
                <Text font="main-ui-body">{provider.name}</Text>
                <Text font="secondary-body" color="text-03">
                  {`${
                    provider.model_count
                  } models${provider.api ? ` · ${provider.api}` : ""}`}
                </Text>
              </Section>
              <Button
                type="button"
                prominence="secondary"
                data-testid={`modelsdev-browse-${provider.id}`}
                onClick={() => loadModels(provider)}
              >
                Browse
              </Button>
            </Section>
          ))}
        </Section>
      )}
    </Section>
  );
}
