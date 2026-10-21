/**
 * Firebird PowerToys — i18n module.
 *
 * Merges app-specific messages with the lib's generic messages.
 *
 * Usage in components (reactive):
 * ```svelte
 * <script>
 *   import { t } from '../i18n';
 *   import { localeStore } from '@edujed/jedsvelted-ui/i18n';
 * </script>
 * <h1>{t('query', $localeStore)}</h1>
 * ```
 */

import { get } from "svelte/store";
import {
  localeStore,
  t as libT,
  type Locale,
} from "@edujed/jedsvelted-ui/i18n";
import en from "./locales/en";
import ptBR from "./locales/pt-BR";

/** Message keys specific to this app. */
export type AppMessageKey = keyof typeof en;

/** Shape every app locale must satisfy. */
export type AppMessages = Record<AppMessageKey, string>;

const APP_LOCALES: Record<Locale, AppMessages> = { en, "pt-BR": ptBR };

/**
 * Translates a message key for the given locale.
 * Checks app-specific messages first, then falls back to the lib's messages.
 * Supports `{placeholder}` substitution.
 */
export function t(
  key: AppMessageKey | Parameters<typeof libT>[0],
  params?: Record<string, string | number>,
  locale?: Locale,
): string {
  const currentLocale = locale ?? get(localeStore);
  const appTemplate = (APP_LOCALES[currentLocale] as Record<string, string>)[
    key as string
  ];

  if (appTemplate !== undefined) {
    if (!params) return appTemplate;
    return appTemplate.replace(/\{(\w+)\}/g, (match, name: string) =>
      name in params ? String(params[name]) : match,
    );
  }

  // Fall through to the lib's generic messages
  return libT(key as Parameters<typeof libT>[0], params);
}
