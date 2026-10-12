'use client'

import { Plus, Trash2 } from 'lucide-react'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'

import { CommitTextarea } from '@/components/controls/CommitTextarea'
import { call } from '@/lib/backend'
import { projectKey, refresh, useProject } from '@/lib/queries'
import { commands, type Character, type Term } from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@koharu/ui/components/tabs'

const fields = [
  ['source_name', 'sourceName'],
  ['target_name', 'targetName'],
  ['speech_style', 'speechStyle'],
  ['appearance', 'appearance'],
  ['notes', 'notes'],
] as const

const blank: Character = {
  source_name: '',
  target_name: '',
  speech_style: '',
  appearance: '',
  notes: '',
}

// Project-level tabs live here.
export function ProjectPanel() {
  const { t } = useTranslation()
  return (
    <Tabs defaultValue='characters' className='h-full min-h-0 gap-0'>
      <TabsList variant='line' aria-label={t('project.tabs')} className='w-full shrink-0 px-2'>
        <TabsTrigger value='characters'>{t('project.characters')}</TabsTrigger>
        <TabsTrigger value='terms'>{t('project.terms')}</TabsTrigger>
      </TabsList>
      <TabsContent value='characters' className='min-h-0 overflow-y-auto'>
        <CharactersTab />
      </TabsContent>
      <TabsContent value='terms' className='min-h-0 overflow-y-auto'>
        <TermsTab />
      </TabsContent>
    </Tabs>
  )
}

function CharactersTab() {
  const { t } = useTranslation()
  const characters = useProject().data?.characters ?? []
  const save = (next: Character[]) =>
    void call(commands.setCharacters, next)
      .then(() => refresh(projectKey))
      .catch(() => undefined)

  return (
    <div className='flex flex-col gap-3 p-2.5'>
      <Button variant='secondary' size='sm' onClick={() => save([...characters, blank])}>
        <Plus aria-hidden='true' /> {t('project.add')}
      </Button>
      {characters.length === 0 && (
        <p className='text-[11px] text-muted-foreground'>{t('project.empty')}</p>
      )}
      {characters.map((character, index) => (
        <CharacterEntry
          // Entries have no id; position is their identity and edits keep the order.
          key={index}
          number={index + 1}
          character={character}
          onChange={(next) => save(characters.map((item, i) => (i === index ? next : item)))}
          onDelete={() => save(characters.filter((_, i) => i !== index))}
        />
      ))}
    </div>
  )
}

function CharacterEntry({
  number,
  character,
  onChange,
  onDelete,
}: {
  number: number
  character: Character
  onChange: (character: Character) => void
  onDelete: () => void
}) {
  const { t } = useTranslation()
  const id = useId()
  const name = character.target_name || character.source_name || t('project.unnamed')
  return (
    <fieldset className='flex min-w-0 flex-col gap-1.5 rounded-md border border-border/60 p-2'>
      <legend className='px-1 text-[11px] font-medium'>{t('project.entry', { number })}</legend>
      {fields.map(([key, label]) => (
        <div key={key} className='flex flex-col gap-0.5'>
          <label htmlFor={`${id}-${key}`} className='text-[10px] text-muted-foreground'>
            {t(`project.${label}`)}
          </label>
          <CommitTextarea
            id={`${id}-${key}`}
            rows={1}
            className='max-h-24 min-h-8 w-full min-w-0 resize-y rounded-md bg-background px-1.5 py-1 text-[12px] leading-4 md:text-[12px]'
            value={character[key]}
            onCommit={(value) => onChange({ ...character, [key]: value })}
          />
        </div>
      ))}
      <Button
        variant='ghost'
        size='sm'
        className='self-end'
        aria-label={t('project.delete', { name })}
        onClick={onDelete}
      >
        <Trash2 aria-hidden='true' />
      </Button>
    </fieldset>
  )
}

const termFields = [
  ['source', 'termSource'],
  ['target', 'termTarget'],
  ['note', 'termNote'],
] as const

function TermsTab() {
  const { t } = useTranslation()
  const terms = useProject().data?.terms ?? []
  const save = (next: Term[]) =>
    void call(commands.setTerms, next)
      .then(() => refresh(projectKey))
      .catch(() => undefined)

  return (
    <div className='flex flex-col gap-3 p-2.5'>
      <Button
        variant='secondary'
        size='sm'
        onClick={() => save([...terms, { source: '', target: '', note: '' }])}
      >
        <Plus aria-hidden='true' /> {t('project.addTerm')}
      </Button>
      {terms.length === 0 && (
        <p className='text-[11px] text-muted-foreground'>{t('project.termsEmpty')}</p>
      )}
      {terms.map((term, index) => (
        <TermEntry
          // Entries have no id; position is their identity and edits keep the order.
          key={index}
          number={index + 1}
          term={term}
          onChange={(next) => save(terms.map((item, i) => (i === index ? next : item)))}
          onDelete={() => save(terms.filter((_, i) => i !== index))}
        />
      ))}
    </div>
  )
}

function TermEntry({
  number,
  term,
  onChange,
  onDelete,
}: {
  number: number
  term: Term
  onChange: (term: Term) => void
  onDelete: () => void
}) {
  const { t } = useTranslation()
  const id = useId()
  const name = term.target || term.source || t('project.unnamed')
  return (
    <fieldset className='flex min-w-0 flex-col gap-1.5 rounded-md border border-border/60 p-2'>
      <legend className='px-1 text-[11px] font-medium'>{t('project.termEntry', { number })}</legend>
      {termFields.map(([key, label]) => (
        <div key={key} className='flex flex-col gap-0.5'>
          <label htmlFor={`${id}-${key}`} className='text-[10px] text-muted-foreground'>
            {t(`project.${label}`)}
          </label>
          <CommitTextarea
            id={`${id}-${key}`}
            rows={1}
            className='max-h-24 min-h-8 w-full min-w-0 resize-y rounded-md bg-background px-1.5 py-1 text-[12px] leading-4 md:text-[12px]'
            value={term[key]}
            onCommit={(value) => onChange({ ...term, [key]: value })}
          />
        </div>
      ))}
      <Button
        variant='ghost'
        size='sm'
        className='self-end'
        aria-label={t('project.deleteTerm', { name })}
        onClick={onDelete}
      >
        <Trash2 aria-hidden='true' />
      </Button>
    </fieldset>
  )
}
