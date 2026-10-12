'use client'

import { Eraser, Plus, X } from 'lucide-react'
import { useEffect, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  PreferencePage,
  PreferenceRow,
  PreferenceSection,
  TextField,
} from '@/components/preferences/PreferenceFields'
import type {
  CredentialInput,
  ProviderConfig,
  ProviderPreference,
  ProviderPreferences as ProviderSettings,
} from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'
import { Input } from '@koharu/ui/components/input'

type ConfigWithSetting<T, Key extends PropertyKey> = T extends { settings: infer Settings }
  ? Key extends keyof Settings
    ? [Settings[Key]] extends [never]
      ? never
      : T
    : never
  : never

type BaseUrlConfig = ConfigWithSetting<ProviderConfig, 'base_url'>
type HeadersConfig = ConfigWithSetting<ProviderConfig, 'headers'>

export function ProviderPreferences({
  value,
  onChange,
}: {
  value: ProviderSettings
  onChange: (value: ProviderSettings) => void
}) {
  const { t } = useTranslation()
  return (
    <PreferencePage
      title={t('settings.providers.title')}
      description={t('settings.providers.description')}
    >
      <PreferenceSection
        title={t('settings.providers.connections')}
        description={t('settings.providers.connectionsDescription')}
      >
        {value.entries.filter(isConfigurable).map((entry) => (
          <PreferenceRow
            key={entry.config.provider}
            title={entry.name}
            description={t(`providerDescriptions.${entry.config.provider}`)}
            align='start'
          >
            <div className='grid gap-2'>
              {entry.credential && (
                <CredentialField
                  label={entry.name}
                  value={entry.credential}
                  onChange={(credential) => onChange(replaceEntry(value, { ...entry, credential }))}
                />
              )}
              {hasBaseUrl(entry.config) && (
                <TextField
                  label={t('model.baseUrl')}
                  type='url'
                  value={entry.config.settings.base_url ?? ''}
                  onChange={(base_url) =>
                    onChange(
                      replaceEntry(value, {
                        ...entry,
                        config: withBaseUrl(entry.config, base_url),
                      }),
                    )
                  }
                />
              )}
              {hasHeaders(entry.config) && (
                <HeadersField
                  value={entry.config.settings.headers ?? {}}
                  onChange={(headers) =>
                    onChange(
                      replaceEntry(value, {
                        ...entry,
                        config: withHeaders(entry.config, headers),
                      }),
                    )
                  }
                />
              )}
            </div>
          </PreferenceRow>
        ))}
      </PreferenceSection>
    </PreferencePage>
  )
}

function HeadersField({
  value,
  onChange,
}: {
  value: Record<string, string>
  onChange: (value: Record<string, string>) => void
}) {
  const { t } = useTranslation()
  // Rows are local so a half-typed or duplicate name isn't dropped mid-edit.
  const [rows, setRows] = useState(() => Object.entries(value))
  const update = (next: [string, string][]) => {
    setRows(next)
    onChange(Object.fromEntries(next.filter(([name]) => name.trim() !== '')))
  }
  return (
    <div className='grid gap-1'>
      <span className='text-[10px] text-muted-foreground'>{t('settings.providers.headers')}</span>
      {rows.map(([name, headerValue], index) => (
        <div key={index} className='flex gap-2'>
          <Input
            aria-label={t('settings.providers.headerName')}
            placeholder={t('settings.providers.headerName')}
            autoCapitalize='none'
            spellCheck={false}
            value={name}
            className='h-8 min-w-0 flex-1 text-[12px]'
            onChange={(event) => {
              const next = [...rows]
              next[index] = [event.currentTarget.value, headerValue]
              update(next)
            }}
          />
          <Input
            aria-label={t('settings.providers.headerValue')}
            placeholder={t('settings.providers.headerValue')}
            autoCapitalize='none'
            spellCheck={false}
            value={headerValue}
            className='h-8 min-w-0 flex-1 text-[12px]'
            onChange={(event) => {
              const next = [...rows]
              next[index] = [name, event.currentTarget.value]
              update(next)
            }}
          />
          <Button
            type='button'
            variant='outline'
            size='icon'
            aria-label={t('settings.providers.removeHeader')}
            onClick={() => update(rows.filter((_, i) => i !== index))}
          >
            <X />
          </Button>
        </div>
      ))}
      <Button
        type='button'
        variant='outline'
        size='sm'
        className='w-fit'
        onClick={() => setRows([...rows, ['', '']])}
      >
        <Plus /> {t('settings.providers.addHeader')}
      </Button>
    </div>
  )
}

function CredentialField({
  label,
  value,
  onChange,
}: {
  label: string
  value: CredentialInput
  onChange: (value: CredentialInput) => void
}) {
  const { t } = useTranslation()
  const inputId = useId()
  const [draftValue, setDraftValue] = useState(value.value ?? '')
  useEffect(() => {
    if (value.value !== null) setDraftValue(value.value)
    else if (!value.configured || value.clear) setDraftValue('')
  }, [value.clear, value.configured, value.value])
  const configured = !value.clear && (value.configured || Boolean(draftValue))
  return (
    <div className='grid gap-1'>
      <label htmlFor={inputId} className='text-[10px] text-muted-foreground'>
        {t('settings.providers.credential')}
      </label>
      <div className='flex gap-2'>
        <Input
          id={inputId}
          aria-label={t('settings.providers.credentialLabel', { provider: label })}
          type='text'
          autoComplete='off'
          autoCapitalize='none'
          spellCheck={false}
          value={draftValue}
          placeholder={
            configured ? t('settings.providers.configured') : t('settings.providers.notConfigured')
          }
          className='h-8 min-w-0 flex-1 text-[12px] [-webkit-text-security:disc] [&::placeholder]:[-webkit-text-security:none]'
          onChange={(event) => {
            const draft = event.currentTarget.value
            setDraftValue(draft)
            onChange({ ...value, value: draft || null, clear: false })
          }}
        />
        {configured && (
          <Button
            type='button'
            variant='outline'
            size='icon'
            aria-label={t('settings.providers.clearCredential', { provider: label })}
            onClick={() => {
              setDraftValue('')
              onChange({ configured: false, value: null, clear: true })
            }}
          >
            <Eraser />
          </Button>
        )}
      </div>
    </div>
  )
}

function isConfigurable(entry: ProviderPreference): boolean {
  return entry.credential !== null || hasBaseUrl(entry.config) || hasHeaders(entry.config)
}

function hasBaseUrl(config: ProviderConfig): config is BaseUrlConfig {
  return 'base_url' in config.settings
}

function hasHeaders(config: ProviderConfig): config is HeadersConfig {
  return 'headers' in config.settings
}

function withHeaders(config: ProviderConfig, headers: Record<string, string>): ProviderConfig {
  if (!hasHeaders(config)) return config
  return { ...config, settings: { ...config.settings, headers } } as ProviderConfig
}

function withBaseUrl(config: ProviderConfig, base_url: string): ProviderConfig {
  if (!hasBaseUrl(config)) return config
  return {
    ...config,
    settings: { ...config.settings, base_url: base_url || null },
  } as ProviderConfig
}

function replaceEntry(
  preferences: ProviderSettings,
  replacement: ProviderPreference,
): ProviderSettings {
  return {
    entries: preferences.entries.map((entry) =>
      entry.config.provider === replacement.config.provider ? replacement : entry,
    ),
  }
}
