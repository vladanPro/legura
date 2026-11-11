# Public interface language V0

At `/admin/language`, an administrator chooses English or French as the
default public interface language and can enable the built-in visitor switcher.
The switcher is disabled on new and upgraded installations. It can be enabled
and disabled without a rebuild or plugin download.

When enabled, the header links switch the current public URL between `?lang=en`
and `?lang=fr`. Public navigation carries that choice between the homepage,
post list, post details, and standalone pages. Invalid values and all visitor
overrides while disabled fall back to the configured interface language. The
links work without JavaScript.

Only product labels are translated. Titles, body text, image descriptions and
editor-defined menu labels remain the author's content. The existing content
language annotation and the admin language are unchanged. No post records are
migrated. Admin, setup, login, and error pages are not switched by a visitor.
