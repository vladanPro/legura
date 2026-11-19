# Accounts And Personal Profiles

This guide describes the current `dev` source. The published **0.0.3 Pilot 1**
download has administrator Profile password changes; Editor and Subscriber
self-service was added afterward in PR #166. It is not in that download.
See [installation](installation.md) for source preparation and
[release notes](releases/0.0.3-pilot.1.md) for the published package boundary.

## Choose A Role

| Capability | Administrator | Editor | Subscriber |
| --- | --- | --- | --- |
| Landing after login | Overview | Posts | Own Profile |
| Read own Profile and change own password | Yes | Yes | Yes |
| Create, edit, publish and trash Posts and Pages | Yes | Yes | No |
| Permanently delete content | Yes | No | No |
| Manage media, settings, navigation and appearance | Yes | No | No |
| Add users or change their role/status | Yes | No | No |

Editor access covers the installation's content, not only posts created by that
Editor. Subscriber is a signed-in account, not a paid subscription or a private
content entitlement. Public published content does not require an account.
Unknown roles have no capabilities. Inactive accounts cannot sign in.
The sidebar reflects permissions, but server checks enforce them independently.

## Add An Account

1. Sign in as the installation administrator and open **Account > Users**.
2. Choose **Add user**, enter an email and an initial password, and select
   **Editor** or **Subscriber**.
3. Submit the form. The account appears in the Users table.
4. Give the person their login URL and initial credentials through a trusted
   channel. Ask them to change the initial password in **Account > Profile**.

The password must have 10 to 256 characters. Use a unique password from a
password manager; satisfying the minimum does not make a reused password safe.
The email must be valid and unused. Legura stores a password hash, not the
plaintext password. It does not send an invitation email or verify mailbox
ownership. The administrator cannot create another Administrator through this
form. Public registration is not available.

## Find Or Change An Account

Use **Users** to search by email and filter by role or active/inactive status.
Filters apply before pagination; they remain selected when changing pages.
Open **View account** to change an Editor to Subscriber or vice versa.
The installation administrator cannot be demoted or deactivated through these
controls. There is no account deletion control.

Deactivation prevents further sign-in and revokes that account's existing
sessions. Its content remains in the database. Reactivation requires a new
sign-in; it does not make the old session cookies valid again. Deactivation
does not unpublish content: change publication status separately if needed.

Role checks use the current stored role. An Editor changed to Subscriber loses
content-management access; their own Profile remains accessible.

## Change Your Own Password

1. Sign in and choose **Account > Profile**.
2. Check that the email and role identify your account.
3. Enter your current password, a new password and its confirmation.
4. Choose **Change password and sign out**, then sign in with the new password.

A successful change signs out **all sessions for your account**, including
other browsers. Other people's sessions are unaffected. A wrong current
password or mismatched confirmation shows a field error without changing the
password. Password changes work with page JavaScript enabled or disabled.

Profile is selected by the authenticated session, not a user ID in the URL or
form. It cannot change another account's credentials. Email, role and site
preferences are not editable self-service fields. The administrator's Profile
links to the separate administration language settings; it is not a personal
language preference for every user.

## If You Cannot Sign In

Check the login email and whether the administrator deactivated the account.
Repeated failed attempts can be throttled; wait rather than continually retrying.
There is no email **Forgot password** flow and no administrator UI for resetting
another user's password yet. Do not delete the database or rerun setup to recover
an account.

An operator who controls the server can recover the **installation
administrator** using the documented
[offline password recovery](operator-quickstart.md#recover-an-administrator-password).
Stop the CMS and follow its backup and hidden-input instructions first. That
command is not a general Editor/Subscriber recovery mechanism. Never paste
passwords, setup tokens, session keys or your `.env` into a support issue.

## Verification Boundary

The isolated `-Users` fixture checks account creation, roles/status, own-profile
authorization, password verification, forged IDs, session revocation and
independence from other accounts. Aegis exercises the own-profile forms with
page scripts on and off and captures 320/390-pixel layouts. These gates passed
for PR #166; they are regression evidence, not a security audit or a production
support guarantee. See [the roadmap](roadmap.md) for remaining product work.
