'use client'

import { Check, Plus, Trash2, X } from 'lucide-react'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'

import { CommitTextarea } from '@/components/controls/CommitTextarea'
import { call } from '@/lib/backend'
import { projectKey, refresh, useProject } from '@/lib/queries'
import { commands, type Character, type Proposals, type Term } from '@koharu/bridge/protocol'
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

const emptyProposals: Proposals = {
  characters: [],
  terms: [],
  rejected_characters: [],
  rejected_terms: [],
  use_in_prompts: false,
}

// Approving moves an entry between lists, so sheet, terms and proposals are saved together.
function useReview() {
  const project = useProject().data
  const characters = project?.characters ?? []
  const terms = project?.terms ?? []
  const proposals = project?.proposals ?? emptyProposals
  const save = (next: { characters?: Character[]; terms?: Term[]; proposals?: Proposals }) =>
    void call(
      commands.setContext,
      next.characters ?? characters,
      next.terms ?? terms,
      next.proposals ?? proposals,
    )
      .then(() => refresh(projectKey))
      .catch(() => undefined)
  return { characters, terms, proposals, save }
}

// Project-level tabs live here.
export function ProjectPanel() {
  const { t } = useTranslation()
  const id = useId()
  const { proposals, save } = useReview()
  return (
    <Tabs defaultValue='characters' className='h-full min-h-0 gap-0'>
      <div className='flex shrink-0 items-center gap-1.5 px-2.5 py-1.5'>
        <input
          id={`${id}-use-pending`}
          type='checkbox'
          checked={proposals.use_in_prompts}
          onChange={(event) =>
            save({ proposals: { ...proposals, use_in_prompts: event.target.checked } })
          }
        />
        <label htmlFor={`${id}-use-pending`} className='text-[11px]'>
          {t('project.usePending')}
        </label>
      </div>
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
  const { characters, terms, proposals, save: saveContext } = useReview()
  const save = (next: Character[]) =>
    void call(commands.setCharacters, next)
      .then(() => refresh(projectKey))
      .catch(() => undefined)
  const pending = proposals.characters
  const setPending = (next: Character[], rest: Partial<Proposals> = {}) =>
    saveContext({ proposals: { ...proposals, ...rest, characters: next } })

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
      {pending.length > 0 && <h3 className='text-[11px] font-medium'>{t('project.pending')}</h3>}
      {pending.map((character, index) => (
        <CharacterEntry
          key={`pending-${index}`}
          number={index + 1}
          character={character}
          onChange={(next) => setPending(pending.map((item, i) => (i === index ? next : item)))}
          review={{
            onApprove: () =>
              saveContext({
                characters: [...characters, character],
                terms,
                proposals: { ...proposals, characters: pending.filter((_, i) => i !== index) },
              }),
            onReject: () =>
              setPending(
                pending.filter((_, i) => i !== index),
                { rejected_characters: [...proposals.rejected_characters, character.source_name] },
              ),
          }}
        />
      ))}
      <RejectedList
        sources={proposals.rejected_characters}
        onRestore={(index) =>
          saveContext({
            proposals: {
              ...proposals,
              rejected_characters: proposals.rejected_characters.filter((_, i) => i !== index),
            },
          })
        }
      />
    </div>
  )
}

function CharacterEntry({
  number,
  character,
  onChange,
  onDelete,
  review,
}: {
  number: number
  character: Character
  onChange: (character: Character) => void
  onDelete?: () => void
  review?: Review
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
      <EntryActions name={name} deleteLabel='project.delete' onDelete={onDelete} review={review} />
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
  const { characters, terms, proposals, save: saveContext } = useReview()
  const save = (next: Term[]) =>
    void call(commands.setTerms, next)
      .then(() => refresh(projectKey))
      .catch(() => undefined)
  const pending = proposals.terms
  const setPending = (next: Term[], rest: Partial<Proposals> = {}) =>
    saveContext({ proposals: { ...proposals, ...rest, terms: next } })

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
      {pending.length > 0 && <h3 className='text-[11px] font-medium'>{t('project.pending')}</h3>}
      {pending.map((term, index) => (
        <TermEntry
          key={`pending-${index}`}
          number={index + 1}
          term={term}
          onChange={(next) => setPending(pending.map((item, i) => (i === index ? next : item)))}
          review={{
            onApprove: () =>
              saveContext({
                characters,
                terms: [...terms, term],
                proposals: { ...proposals, terms: pending.filter((_, i) => i !== index) },
              }),
            onReject: () =>
              setPending(
                pending.filter((_, i) => i !== index),
                { rejected_terms: [...proposals.rejected_terms, term.source] },
              ),
          }}
        />
      ))}
      <RejectedList
        sources={proposals.rejected_terms}
        onRestore={(index) =>
          saveContext({
            proposals: {
              ...proposals,
              rejected_terms: proposals.rejected_terms.filter((_, i) => i !== index),
            },
          })
        }
      />
    </div>
  )
}

function TermEntry({
  number,
  term,
  onChange,
  onDelete,
  review,
}: {
  number: number
  term: Term
  onChange: (term: Term) => void
  onDelete?: () => void
  review?: Review
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
      <EntryActions
        name={name}
        deleteLabel='project.deleteTerm'
        onDelete={onDelete}
        review={review}
      />
    </fieldset>
  )
}

type Review = { onApprove: () => void; onReject: () => void }

// Pending entries are edited in place and then approved or rejected; approved ones can be deleted.
function EntryActions({
  name,
  deleteLabel,
  onDelete,
  review,
}: {
  name: string
  deleteLabel: string
  onDelete?: () => void
  review?: Review
}) {
  const { t } = useTranslation()
  if (!review) {
    return (
      <Button
        variant='ghost'
        size='sm'
        className='self-end'
        aria-label={t(deleteLabel, { name })}
        onClick={onDelete}
      >
        <Trash2 aria-hidden='true' />
      </Button>
    )
  }
  return (
    <div className='flex justify-end gap-1'>
      <Button
        variant='ghost'
        size='sm'
        aria-label={t('project.reject', { name })}
        onClick={review.onReject}
      >
        <X aria-hidden='true' />
      </Button>
      <Button
        variant='secondary'
        size='sm'
        aria-label={t('project.approve', { name })}
        onClick={review.onApprove}
      >
        <Check aria-hidden='true' /> {t('project.approveShort')}
      </Button>
    </div>
  )
}

// Rejected sources are never proposed again until restored; the list stays collapsed.
function RejectedList({
  sources,
  onRestore,
}: {
  sources: string[]
  onRestore: (index: number) => void
}) {
  const { t } = useTranslation()
  if (sources.length === 0) return null
  return (
    <details className='text-[11px]'>
      <summary className='cursor-pointer'>
        {t('project.rejected', { count: sources.length })}
      </summary>
      <ul className='mt-1 flex flex-col gap-1'>
        {sources.map((source, index) => (
          <li key={`${source}-${index}`} className='flex items-center justify-between gap-2'>
            <span className='min-w-0 truncate'>{source}</span>
            <Button
              variant='ghost'
              size='sm'
              aria-label={t('project.restore', { name: source })}
              onClick={() => onRestore(index)}
            >
              {t('project.restoreShort')}
            </Button>
          </li>
        ))}
      </ul>
    </details>
  )
}
