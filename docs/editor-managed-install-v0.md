# Editor Managed Installation V0

This build-time helper installs the pinned editor package side by side. It does
not turn on rich editing in the operator CMS, migrate its database or change
application imports. Run it against a trusted source checkout, not a live public
document root, and finish installation before building/deploying the application.

```powershell
cargo run --locked --bin legura-editor-assets -- check /path/to/project
cargo run --locked --bin legura-editor-assets -- install /path/to/project --json
```

`check` is read-only. `install` writes only a new generation derived from the
package version, layout and exact file hashes. The report provides a component
path, stylesheet URL and browser module URL. The fixed destinations are:

```text
app/components/legura-editor/<sha256>/RichTextEditor.asx
public/content-editor/<sha256>/editor.css
public/content-editor/<sha256>/*.js
.legura/editor-assets/<sha256>.json
```

The component must be imported explicitly. Application lifecycle glue remains
outside the generation and imports the reported module URL. Relative imports
between the package's three browser modules remain inside the same generation.
The browser proof uses these exact installed files, not a copied component alias.

## Ownership And Conflicts

The receipt is written last. It is compared with the compiled package inventory,
not treated as trusted input. A repeated install verifies the receipt and every
file byte; unchanged generations are a no-op. Missing/edited files, extra entries,
changed receipts and incomplete/unowned generations fail without repairing or
overwriting them. Linked descendants (including Windows reparse points) are
refused. The explicitly selected project root is canonicalized.

An updated package gets a new generation. Existing versions and customizations
are left alone; there is no automatic active pointer, force flag, deletion,
garbage collection, overwrite or rollback operation. Copy custom components out
of the managed generation before adapting them. Old versions are not audited by
the new package's check; check verifies only the package compiled into the tool.

This is not a multi-file filesystem transaction. A failed/interrupted write can
leave an incomplete generation; a subsequent run refuses it for manual inspection
rather than guessing recovery or deleting data. Install only while the checkout
is exclusively owned by the operator: concurrent writers or malicious filesystem
replacement races are outside V0. Hashes identify content, not publisher identity
or a signed authenticity proof. Keep receipts in source/build metadata, not public.

## Acceptance And Remaining Gates

Library tests cover read-only planning, repeat installation, edited files,
unknown files, incomplete generations, changed receipts and Unix symlink refusal.
The disposable CMS browser fixture explicitly binds the reported component/CSS/JS
paths and then verifies the installed generation before building.

Operator activation, configuration compatibility, registry native 422 acceptance,
editor licensing/publication and a recoverable upgrade/deployment procedure are
still separate gates. This is not a production CMS installer.

Local Windows acceptance: 25 library tests, strict Clippy, formatting and
registry PostEditor JS/no-JS browser flows passed. The JS flow was rerun after
sharing the inventory calculation with the existing bundle exporter. Linux
symlink and source NativeRetry checks remain required CI merge gates.
