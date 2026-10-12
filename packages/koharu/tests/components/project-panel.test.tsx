import { QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ProjectPanel } from '@/components/editor/ProjectPanel'
import { projectKey, queryClient } from '@/lib/queries'
import { commands, type Character, type ProjectInfo, type Term } from '@koharu/bridge/protocol'

const alice: Character = {
  source_name: 'アリス',
  target_name: 'Alice',
  speech_style: 'formal',
  appearance: 'blonde',
  notes: 'rival of Bob',
}

const demon: Term = { source: '魔王', target: 'Demon King', note: 'title' }

function install(characters: Character[], terms: Term[] = []) {
  const project: ProjectInfo = {
    name: 'Book',
    revision: 1,
    active_page: null,
    can_undo: false,
    can_redo: false,
    characters,
    terms,
  }
  queryClient.setQueryData(projectKey, project)
  vi.spyOn(commands, 'getProject').mockResolvedValue(project)
  render(
    <QueryClientProvider client={queryClient}>
      <ProjectPanel />
    </QueryClientProvider>,
  )
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
})
