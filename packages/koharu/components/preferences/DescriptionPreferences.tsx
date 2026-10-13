'use client'

import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ModelPicker } from '@/components/controls/ModelPicker'
import { GenerationPreferences } from '@/components/preferences/GenerationPreferences'
import {
  NumberField,
  PreferencePage,
  PreferenceRow,
  PreferenceSection,
} from '@/components/preferences/PreferenceFields'
import { modelKey, providerName } from '@/lib/translation'
import type {
  Model,
  ProviderPreference,
  TranslationConfig as TranslationSettings,
} from '@koharu/bridge/protocol'
import { Badge } from '@koharu/ui/components/badge'
import { Button } from '@koharu/ui/components/button'
import { Popover, PopoverContent, PopoverTrigger } from '@koharu/ui/components/popover'
import { Textarea } from '@koharu/ui/components/textarea'

// Mirrors TranslationPreferences for the description call. There is no language control: the
// description is written in the Translation page's target language. There is no reasoning
// control either: reasoning is never sent for the description.
export function DescriptionPreferences({
  value,
  modelChoices,
  providers,
  onChange,
}: {
  value: TranslationSettings
  modelChoices: Model[]
  providers: ProviderPreference[]
  onChange: (value: TranslationSettings) => void
}) {
  const { t } = useTranslation()
  const [modelOpen, setModelOpen] = useState(false)
  const selection = value.description_model ?? null
  const selected = selection
    ? (modelChoices.find((candidate) => modelKey(candidate) === modelKey(selection)) ?? null)
    : null
  const current: Model | null = selection
    ? (selected ?? {
        ...selection,
        model: selection.model ?? null,
        name: selection.model ?? providerName(providers, selection.provider),
        quantizations: [],
        vision: selection.vision ?? false,
        reasoning: selection.reasoning ?? false,
      })
    : null
  const choices = selected || !current ? modelChoices : [current, ...modelChoices]
  const quantization =
    current?.quantizations.find((candidate) => candidate.id === selection?.quantization) ??
    current?.quantizations[0]
  return (
    <PreferencePage
      title={t('settings.description.title')}
      description={t('settings.description.description')}
    >
      <PreferenceSection
        title={t('settings.description.model')}
        description={t('settings.description.modelDescription')}
      >
        <PreferenceRow
          title={t('settings.description.descriptionModel')}
          description={t('settings.description.descriptionModelDescription')}
        >
          <div className='flex flex-col items-start gap-2'>
            <Popover open={modelOpen} onOpenChange={setModelOpen}>
              <PopoverTrigger
                type='button'
                aria-label={t('settings.description.descriptionModel')}
                className='flex h-9 w-full min-w-0 items-center justify-between gap-2 rounded-lg border border-input bg-transparent px-2.5 text-[11px] transition-colors outline-none hover:bg-foreground/[0.03] focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50'
              >
                <span className='min-w-0 flex-1 text-left'>
                  {current ? (
                    <span className='flex min-w-0 items-center gap-2'>
                      <Badge
                        variant='outline'
                        className='shrink-0 px-1.5 py-0 text-[9px] font-medium'
                      >
                        {providerName(providers, current.provider)}
                      </Badge>
                      <span className='truncate'>
                        {current.name}
                        {(quantization?.name ?? selection?.quantization) && (
                          <span className='text-muted-foreground'>
                            {' · '}
                            {quantization?.name ?? selection?.quantization}
                          </span>
                        )}
                      </span>
                    </span>
                  ) : (
                    <span className='text-muted-foreground'>
                      {t('settings.description.noModel')}
                    </span>
                  )}
                </span>
                <ChevronDown className='size-3.5 shrink-0 text-muted-foreground' />
              </PopoverTrigger>
              <PopoverContent
                align='start'
                sideOffset={4}
                className='w-(--anchor-width) min-w-64 gap-0 overflow-hidden rounded-xl border border-border/50 p-1 shadow-sm ring-0'
              >
                <ModelPicker
                  value={selection}
                  models={choices}
                  providers={providers}
                  onBack={() => setModelOpen(false)}
                  onSelect={(description_model) => {
                    onChange({ ...value, description_model })
                    setModelOpen(false)
                  }}
                />
              </PopoverContent>
            </Popover>
            <Button
              type='button'
              variant='ghost'
              size='sm'
              disabled={!selection}
              className='h-7 px-2 text-[11px]'
              onClick={() => onChange({ ...value, description_model: null })}
            >
              {t('settings.description.clear')}
            </Button>
          </div>
        </PreferenceRow>
      </PreferenceSection>

      <GenerationPreferences
        value={value.description_generation ?? { reasoning: null }}
        showReasoning={false}
        showVision={false}
        onChange={(description_generation) => onChange({ ...value, description_generation })}
      />

      <PreferenceSection
        title={t('settings.translation.output')}
        description={t('settings.description.outputDescription')}
      >
        <PreferenceRow
          title={t('model.instructions')}
          description={t('settings.description.instructionsDescription')}
          align='start'
        >
          <Textarea
            aria-label={t('settings.description.instructionsLabel')}
            value={value.description_instructions ?? ''}
            className='field-sizing-fixed min-h-24 resize-y overflow-y-auto text-[12px] leading-5'
            placeholder={t('settings.description.instructionsPlaceholder')}
            onChange={(event) =>
              onChange({ ...value, description_instructions: event.currentTarget.value || null })
            }
          />
        </PreferenceRow>
        <PreferenceRow
          title={t('settings.description.previousPages')}
          description={t('settings.description.previousPagesDescription')}
        >
          <NumberField
            label={t('settings.description.previousPages')}
            value={value.description_previous_pages ?? 0}
            min={0}
            max={10}
            step={1}
            onChange={(pages) =>
              pages !== null &&
              onChange({ ...value, description_previous_pages: Math.trunc(pages) })
            }
          />
        </PreferenceRow>
      </PreferenceSection>
    </PreferencePage>
  )
}
