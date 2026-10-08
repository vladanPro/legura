# Installation Model

Planned distribution paths; none is implemented yet.

## Prebuilt Native Package

Download a package matching the server OS/architecture and supported system
libraries. Upload it using SFTP or another deployment method. Set execution
permissions, configure storage/database/secrets, and start it as a supervised
service behind the hosting provider's reverse proxy and TLS configuration.

Uploading files alone does not start the service. A Windows executable does not
run on a Linux host. Rust is not required to run a compatible prebuilt binary.

## Source Build

Administrators may install Rust/Cargo and required native build dependencies,
compile on the server or a matching build machine, and run the resulting binary.
Compilation has different memory/CPU requirements from serving requests.
Document the actual build command once the application's build pipeline exists.

## Docker

An optional alternative with the same application behavior and database/storage
contracts. It is not a prerequisite for running Legura.

## Hosting Boundary

Shared hosting is suitable only when its provider permits and supervises native
application processes and routes web traffic to them. Installing Rust does not
remove provider restrictions. Providers can build a release once and run isolated
instances with separate configuration and persistent data.

## Browser Setup

Choose a supported database, set site details, create the first administrator,
and complete installation. Setup must lock after completion and must not leak
database credentials or allow unauthenticated administrator replacement.

## Upgrades

Program files are replaceable; configuration, secrets, uploads, and databases
are persistent. Back up first, execute checked migrations, verify health, and
document recovery for both application and schema changes. Never overwrite user
data when uploading a new version.
