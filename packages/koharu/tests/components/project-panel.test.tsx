import { QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ProjectPanel } from '@/components/editor/ProjectPanel'
import { RightSidebar } from '@/components/editor/RightSidebar'
import { projectKey, queryClient } from '@/lib/queries'
import {
  commands,
  type Character,
  type ProjectInfo,
  type Proposals,
  type Term,
} from '@koharu/bridge/protocol'

const emptyProposals: Proposals = {
  characters: [],
  terms: [],
  character_updates: [],
  term_updates: [],
  rejected_characters: [],
  rejected_terms: [],
  use_in_prompts: false,
}

const alice: Character = {
  source_name: 'アリス',
  target_name: 'Alice',
  speech_style: 'formal',
  appearance: 'blonde',
  notes: 'rival of Bob',
}

const demon: Term = { source: '魔王', target: 'Demon King', note: 'title' }

function install(
  characters: Character[],
  terms: Term[] = [],
  proposals: Proposals = emptyProposals,
  ui = <ProjectPanel />,
) {
  const project: ProjectInfo = {
    name: 'Book',
    revision: 1,
    active_page: null,
    can_undo: false,
    can_redo: false,
    characters,
    terms,
    proposals,
  }
  queryClient.setQueryData(projectKey, project)
  vi.spyOn(commands, 'getProject').mockResolvedValue(project)
  render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>)
}

describe('project panel', () => {
  it('shows a Characters tab with every field labelled', () => {
    install([alice])

    expect(screen.getByRole('tab', { name: 'Characters', selected: true })).toBeInTheDocument()
    expect(screen.getByLabelText('Source name')).toHaveValue('アリス')
    expect(screen.getByLabelText('Target name')).toHaveValue('Alice')
    expect(screen.getByLabelText('Speech style')).toHaveValue('formal')
    expect(screen.getByLabelText('Appearance')).toHaveValue('blonde')
    expect(screen.getByLabelText('Notes')).toHaveValue('rival of Bob')
  })

  it('adds, edits and deletes entries with the keyboard alone', async () => {
    const user = userEvent.setup()
    const setCharacters = vi.spyOn(commands, 'setCharacters').mockResolvedValue(null)
    install([alice])

    const add = screen.getByRole('button', { name: 'Add character' })
    for (let i = 0; i < 4 && document.activeElement !== add; i++) await user.tab()
    expect(add).toHaveFocus()
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(setCharacters).toHaveBeenLastCalledWith([
        alice,
        { source_name: '', target_name: '', speech_style: '', appearance: '', notes: '' },
      ]),
    )

    await user.tab() // first entry: source name
    await user.tab() // target name
    await user.keyboard('{Control>}a{/Control}Alicia')
    await user.tab() // blur commits
    await waitFor(() =>
      expect(setCharacters).toHaveBeenLastCalledWith([{ ...alice, target_name: 'Alicia' }]),
    )

    const remove = screen.getByRole('button', { name: 'Delete Alice' })
    remove.focus()
    await user.keyboard('{Enter}')
    await waitFor(() => expect(setCharacters).toHaveBeenLastCalledWith([]))
  })

  it('adds, edits and deletes terms from the Term list tab with the keyboard alone', async () => {
    const user = userEvent.setup()
    const setTerms = vi.spyOn(commands, 'setTerms').mockResolvedValue(null)
    install([], [demon])

    const tab = screen.getByRole('tab', { name: 'Term list' })
    tab.focus()
    await user.keyboard('{Enter}')
    expect(tab).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByLabelText('Source')).toHaveValue('魔王')
    expect(screen.getByLabelText('Target')).toHaveValue('Demon King')
    expect(screen.getByLabelText('Note (optional)')).toHaveValue('title')

    const add = screen.getByRole('button', { name: 'Add term' })
    add.focus()
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(setTerms).toHaveBeenLastCalledWith([demon, { source: '', target: '', note: '' }]),
    )

    screen.getByLabelText('Target').focus()
    await user.keyboard('{Control>}a{/Control}Maou')
    await user.tab() // blur commits
    await waitFor(() => expect(setTerms).toHaveBeenLastCalledWith([{ ...demon, target: 'Maou' }]))

    screen.getByRole('button', { name: 'Delete Demon King' }).focus()
    await user.keyboard('{Enter}')
    await waitFor(() => expect(setTerms).toHaveBeenLastCalledWith([]))
  })

  it('approves, edits and rejects pending entries with the keyboard alone', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    const bob: Character = { ...alice, source_name: 'ボブ', target_name: 'Bob' }
    const proposals = { ...emptyProposals, characters: [bob, { ...alice, source_name: 'カイ' }] }
    install([alice], [], proposals)

    expect(screen.getByRole('heading', { name: 'Pending review' })).toBeInTheDocument()
    // Edit in place: commits the pending list only, nothing is approved.
    const [, bobTarget] = screen.getAllByLabelText('Target name')
    bobTarget.focus()
    await user.keyboard('{Control>}a{/Control}Robert')
    await user.tab()
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([alice], [], {
        ...proposals,
        characters: [{ ...bob, target_name: 'Robert' }, proposals.characters[1]],
      }),
    )

    screen.getByRole('button', { name: 'Approve Bob' }).focus()
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([alice, bob], [], {
        ...proposals,
        characters: [proposals.characters[1]],
      }),
    )

    screen.getByRole('button', { name: 'Reject Bob' }).focus()
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([alice], [], {
        ...proposals,
        characters: [proposals.characters[1]],
        rejected_characters: ['ボブ'],
      }),
    )
  })

  it('rejects pending terms into a collapsed list and restores them', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    const proposals = {
      ...emptyProposals,
      terms: [demon],
      rejected_terms: ['勇者'],
    }
    install([], [], proposals)
    await user.click(screen.getByRole('tab', { name: 'Term list' }))

    const summary = screen.getByText('Rejected (1)')
    const details = summary.closest('details')!
    expect(details.open).toBe(false)
    expect(screen.getByText('勇者')).not.toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Reject Demon King' }))
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([], [], {
        ...proposals,
        terms: [],
        rejected_terms: ['勇者', '魔王'],
      }),
    )

    await user.click(summary)
    expect(screen.getByText('勇者')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Restore 勇者' }))
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([], [], { ...proposals, rejected_terms: [] }),
    )
  })

  it('approves a pending term into the term list', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    install([], [], { ...emptyProposals, terms: [demon] })
    await user.click(screen.getByRole('tab', { name: 'Term list' }))
    await user.click(screen.getByRole('button', { name: 'Approve Demon King' }))
    await waitFor(() => expect(setContext).toHaveBeenLastCalledWith([], [demon], emptyProposals))
  })

  it('shows old and new values of changed fields only, and approves just those', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    const update = {
      source_name: 'アリス',
      target_name: null,
      speech_style: 'curt',
      appearance: null,
      notes: null,
    }
    const proposals = { ...emptyProposals, character_updates: [update] }
    install([alice], [], proposals)

    const row = screen.getByRole('group', { name: 'Update to アリス' })
    expect(row).toHaveTextContent('formal') // old value
    const style = within(row).getByLabelText('Speech style')
    expect(style).toHaveValue('curt') // new value
    expect(within(row).queryByLabelText('Appearance')).not.toBeInTheDocument() // unchanged

    // Edit the new value before approving.
    style.focus()
    await user.keyboard('{Control>}a{/Control}terse')
    await user.tab()
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([alice], [], {
        ...proposals,
        character_updates: [{ ...update, speech_style: 'terse' }],
      }),
    )

    screen.getByRole('button', { name: 'Approve アリス' }).focus()
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([{ ...alice, speech_style: 'curt' }], [], {
        ...proposals,
        character_updates: [],
      }),
    )
  })

  it('rejecting an update just deletes it, and term updates approve the same way', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    const update = { source: '魔王', target: 'Dark Lord', note: null }
    const proposals = { ...emptyProposals, term_updates: [update] }
    install([], [demon], proposals)
    await user.click(screen.getByRole('tab', { name: 'Term list' }))

    expect(screen.getByRole('group', { name: 'Update to 魔王' })).toHaveTextContent('Demon King')
    await user.click(screen.getByRole('button', { name: 'Approve 魔王' }))
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([], [{ ...demon, target: 'Dark Lord' }], {
        ...proposals,
        term_updates: [],
      }),
    )
    await user.click(screen.getByRole('button', { name: 'Reject 魔王' }))
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([], [demon], {
        ...proposals,
        term_updates: [],
      }),
    )
  })

  it('toggles using pending entries in prompts', async () => {
    const user = userEvent.setup()
    const setContext = vi.spyOn(commands, 'setContext').mockResolvedValue(null)
    install([], [])
    const toggle = screen.getByRole('checkbox', { name: 'Use pending entries in prompts' })
    expect(toggle).not.toBeChecked()
    await user.click(toggle)
    await waitFor(() =>
      expect(setContext).toHaveBeenLastCalledWith([], [], {
        ...emptyProposals,
        use_in_prompts: true,
      }),
    )
  })

  it('shows the pending count as a badge on the Project button', () => {
    install(
      [],
      [],
      {
        ...emptyProposals,
        characters: [alice],
        terms: [demon, demon],
        character_updates: [
          { source_name: 'a', target_name: 'x', speech_style: null, appearance: null, notes: null },
        ],
        term_updates: [{ source: 'b', target: 'y', note: null }],
      },
      <RightSidebar />,
    )
    expect(screen.getByRole('button', { name: /5 pending entries/ })).toBeInTheDocument()
  })

  it('shows no badge when nothing is pending', () => {
    install([], [], emptyProposals, <RightSidebar />)
    expect(screen.queryByText(/pending entries$/)).not.toBeInTheDocument()
  })
})
