<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    EditorView,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
  } from "@codemirror/view";
  import { EditorState } from "@codemirror/state";
  import { sql, SQLDialect } from "@codemirror/lang-sql";
  import { autocompletion } from "@codemirror/autocomplete";
  import { SQL_KEYWORDS } from "./sqlKeywords";

  let {
    value = $bindable(""),
    onValueChange = () => {},
    onRun = () => {},
  }: {
    value?: string;
    onValueChange?: (v: string) => void;
    onRun?: () => void;
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | null = null;

  // Firebird dialect: double-quoted identifiers + Firebird keywords.
  // identifierQuotes defaults to `"` which is correct for Firebird.
  const firebirdDialect = SQLDialect.define({
    keywords: SQL_KEYWORDS.join(" "),
  });

  function buildState(initial: string): EditorState {
    return EditorState.create({
      doc: initial,
      extensions: [
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        sql({ dialect: firebirdDialect }),
        // v1: keyword-only autocomplete (table/column = phase 2).
        autocompletion({
          override: [
            (context) => {
              const word = context.matchBefore(/[\w$]+/);
              if (!word && !context.explicit) return null;
              const from = word ? word.from : context.state.selection.main.head;
              return {
                from,
                options: SQL_KEYWORDS.map((k) => ({ label: k, type: "keyword", apply: k })),
                validFor: /^[\w$]*$/,
              };
            },
          ],
        }),
        keymap.of([
          {
            key: "Mod-Enter",
            run: () => {
              onRun();
              return true;
            },
          },
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            const newText = update.state.doc.toString();
            onValueChange(newText);
            value = newText;
          }
        }),
      ],
    });
  }

  onMount(() => {
    view = new EditorView({
      parent: container,
      state: buildState(value),
    });
  });

  onDestroy(() => {
    view?.destroy();
    view = null;
  });

  // External value changes (e.g. "clear" button) — only when different.
  $effect(() => {
    if (view && value !== view.state.doc.toString()) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    }
  });
</script>

<div class="fb-sql-editor" bind:this={container}></div>
