# Modula Privacy Policy

**Last updated: September 27, 2026**

This policy covers the modula.sh website, the Modula desktop application, and
Modula - AI Development, our app for iPhone and iPad. In short:

- **Modula runs on your machine.** We operate no service that receives your
  projects, prompts, code, or agent activity.
- **The iOS App talks only to a computer you run.** That connection is
  encrypted end to end, and only your own devices can read it.
- **Neither application sends us telemetry.** There is no analytics or
  crash-reporting service in either one, and no Modula account.
- **Website analytics are the only personal information we handle** — see
  Section 2, and you can opt out.

## 1. Who we are

Modula is the name of a software project, not a company. "We," "us," and "our"
mean the Modula Parties as defined in the [Terms of
Use](https://modula.sh/terms). In this policy, the "App" is the Modula desktop
application, the "iOS App" is our app for iPhone and iPad, distributed on the
App Store as "Modula - AI Development," and your "host" is a computer of yours
running the App that you have paired with the iOS App.

Third parties you bring in — an AI model provider you configure, an app store,
your network — handle information under their own policies, not this one.

## 2. What we collect

| Where | What |
| --- | --- |
| This website | Google Analytics records the pages you view, your browser and device type, the link that referred you, and an approximate location derived from your IP address, and sets cookies to tell one visit from another. There are no accounts, forms, or newsletters. |
| The App | Nothing reaches us. It contains no analytics or telemetry. It checks for software updates, and it talks to the AI providers and tools you configure using your own credentials — your prompts and code go to them, under their policies. |
| The iOS App | Nothing reaches us. It contains no analytics, crash-reporting, or advertising code, and no account. What it holds stays on your device; what it sends goes to your host. It also fetches this policy and the Terms of Use from GitHub so they are always current. |

You can block or delete analytics cookies in your browser settings, or install
Google's [opt-out add-on](https://tools.google.com/dlpage/gaoptout). The site
works normally either way, and your light or dark theme preference is stored
on your device and never transmitted.

### The iOS App in more detail

On your device, the app keeps a copy of the workspace your host has sent it —
tasks, runs, and conversations — so it works offline, together with a device
key held in the iOS Keychain that identifies your phone to your host and to
nothing else. Deleting the app removes all of it. Your remote password is used
inside the encrypted session and is not stored.

The camera is used for one thing: reading the pairing code your host displays.
Frames are decoded and discarded, and nothing is recorded or transmitted.

Traffic to your host is encrypted end to end. When your two devices cannot
connect directly, it travels through a relay — ours, or a public one operated
by a third party — which carries encrypted data it cannot read and keeps no
copy. If your host has moved network since you paired, the app looks up where
to reach it through a third-party directory service; that lookup concerns your
host, not your content, and the app publishes nothing about your device to it.

When you open this policy or the Terms of Use inside the app, it downloads the
current text from our public repository on GitHub, so what you read is never a
stale copy shipped months earlier. That request reaches GitHub, not us, and it
carries nothing about you beyond the network metadata any web request carries.

If you have turned on *Share With App Developers* in your iOS settings, Apple
may forward crash reports to us through its own pipeline. Those describe a
crash, not your workspace.

## 3. What we never collect

- Your code, repositories, prompts, or agent output.
- Credentials of any kind.
- Contacts, photos, health data, precise location, or payment information.
- Advertising identifiers. We run no advertising and do not track you across
  apps or websites.

**We do not sell personal information, and we do not share it for
cross-context behavioural advertising.**

## 4. Why, and our legal bases

We use what Section 2 describes to understand which pages of the site people
find useful, to distribute and update the software, to let your own devices
reach each other, and to answer messages you send us. We do not profile you,
make automated decisions about you, or train models on your data. Where the UK
or EU GDPR applies, our bases are your **consent** for analytics cookies and
similar technologies, our **legitimate interests** in operating and securing
Modula, and **performance of a contract** for providing the software under the
[Terms of Use](https://modula.sh/terms). You may withdraw consent or object at
any time.

## 5. Who else is involved

| Provider | Role |
| --- | --- |
| Google | Website analytics. |
| GitHub | Hosts the source code, the downloads, the update information, and the text of this policy and the Terms of Use. |
| Apple | Distributes the iOS App and, if you opted in, forwards crash reports. |
| Our hosting and networking providers | Run the relay infrastructure that carries encrypted traffic between your devices. |

None of them receives your code, credentials, or workspace, and each is bound
by privacy commitments we consider to give your information protection
equivalent to this policy. We may disclose information where the law requires
it, or to establish or defend legal claims — though there is rarely anything
to disclose, since we never receive the things that would matter.

## 6. Retention

- **On your devices** — until you delete the app or its data.
- **In transit** — encrypted traffic is forwarded as it arrives and never
  stored.
- **Website analytics** — retained by Google for the period configured on our
  property, then deleted. We keep no separate copy.
- **Email you send us** — as long as needed to deal with it.

## 7. International transfers

Our providers are international, so the information in Section 2 may be
processed outside the country you live in, including in the United States.
Where the UK or EU GDPR applies, those transfers rely on the safeguards our
providers have in place, such as Standard Contractual Clauses or an applicable
adequacy decision. Traffic between your own devices is encrypted wherever it
travels.

## 8. Your rights and choices

Whoever and wherever you are, you can:

- opt out of website analytics, as described in Section 2;
- delete everything local by removing the iOS App from your device, or the App
  and its data from your computer;
- end a pairing from either side, or turn off remote access on your host
  entirely;
- stop Apple crash reports by turning off *Share With App Developers*.

If the UK or EU GDPR applies to you, you have the right to access, correct,
erase, restrict, object to, and receive a portable copy of your personal data,
and to withdraw consent. Write to [legal@modula.sh](mailto:legal@modula.sh).
You may also complain to your local supervisory authority, or the Information
Commissioner's Office in the United Kingdom, though we would appreciate the
chance to help first. Note what is possible: we hold no account and no user
database, analytics reach us only in aggregate and cannot be connected to you,
and data on your own devices is under your control rather than ours.

If you live in California or another state with comparable law, you have the
right to know what personal information is collected, used, and disclosed, to
have it deleted or corrected, and not to be treated differently for asking. In
the twelve months before the date above we collected the internet and network
activity described in Section 2 for analytics, and disclosed it to the
provider named in Section 5. To make a request, write to
[legal@modula.sh](mailto:legal@modula.sh); we will verify it by corresponding
with you there, and you may use an authorized agent.

## 9. Children

Modula is a developer tool and is not directed to children. We do not
knowingly collect personal information from anyone under 13, or under the age
of digital consent where that is higher. If you believe we have, write to
[legal@modula.sh](mailto:legal@modula.sh) and we will delete it.

## 10. Security

Modula is built so that there is as little to protect as possible: connections
between your devices are encrypted end to end, your device key is held in the
iOS Keychain, and your credentials never leave your machine. No system is
perfectly secure and we cannot guarantee absolute security. If you believe you
have found a vulnerability, please write to
[legal@modula.sh](mailto:legal@modula.sh) and give us a reasonable opportunity
to address it before disclosing it publicly.

## 11. Changes

We may update this policy as Modula changes. We will revise the date above
and, for material changes, seek your consent where the law requires it.
Continuing to use Modula after a change takes effect means you accept the
updated policy.

## 12. Contact

**[legal@modula.sh](mailto:legal@modula.sh)**
