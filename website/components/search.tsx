'use client';
import {
  SearchDialog,
  SearchDialogClose,
  SearchDialogContent,
  SearchDialogHeader,
  SearchDialogIcon,
  SearchDialogInput,
  SearchDialogList,
  SearchDialogOverlay,
  type SharedProps,
} from 'fumadocs-ui/components/dialog/search';
import { useStaticSearch } from 'fumadocs-core/search/client';
import { useI18n } from 'fumadocs-ui/contexts/i18n';

export interface StaticSearchDialogProps extends SharedProps {
  /**
   * URL of the pre-rendered static search index (`/api/search`).
   */
  api?: string;
}

export default function DefaultSearchDialog({
  api = '/api/search',
  ...props
}: StaticSearchDialogProps) {
  const { locale } = useI18n();
  const search = useStaticSearch({
    from: api,
    locale,
  });

  return (
    <SearchDialog {...search} {...props}>
      <SearchDialogOverlay />
      <SearchDialogContent>
        <SearchDialogHeader>
          <SearchDialogIcon />
          <SearchDialogInput />
          <SearchDialogClose />
        </SearchDialogHeader>
        <SearchDialogList />
      </SearchDialogContent>
    </SearchDialog>
  );
}
