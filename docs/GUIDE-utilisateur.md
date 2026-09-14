# Guide utilisateur — Lumen Community

> Tout ce que vous pouvez faire avec Lumen et comment l'utiliser au quotidien.
> Destiné aux utilisateurs finaux et aux administrateurs.

---

## 1. C'est quoi Lumen ?

Lumen est un **assistant IA d'entreprise** : il se connecte à vos outils
(Google Drive, Slack, Confluence, GitHub, SharePoint…), indexe vos documents
et vous permet de **discuter avec vos connaissances** via un LLM (OpenAI,
Anthropic, ou en local via **Ollama**). Tout reste **chez vous** : auto-hébergé.

---

## 2. Premier pas

1. Ouvrez l'URL fournie par votre admin (ex. `https://lumen.maboite.com`).
2. **Inscrivez-vous** avec votre email professionnel (ou connectez-vous via le
   SSO de l'entreprise : Google / OIDC / SAML).
3. Le premier utilisateur inscrit devient automatiquement **administrateur**.

---

## 3. L'interface en un coup d'œil

| Zone | Ce que vous y trouvez |
|---|---|
| **Barre latérale gauche** | Nouvelle session, historique de chats, agents, projets |
| **Barre de saisie** | Votre message + pièces jointes (+ drag & drop) |
| **Sélecteur de modèle** | Choix du LLM (ex. « Llama 3.1 8B » via Ollama) |
| **Outils (icône outils)** | Recherche web, génération d'images, lecture de fichiers… |
| **Admin Panel** | Réservé aux administrateurs (roue dentée / menu admin) |

---

## 4. Discuter (le cœur du produit)

### Chat simple
- Tapez votre question → la réponse est streamée en direct.
- **Joindre des fichiers** : glissez-déposez ou 📎 — les fichiers sont indexés
  et l'agent peut les lire (PDF, DOCX, TXT, images, Excel…).
- **Images, audio, vidéo** : si le modèle sélectionné les accepte (affiché
  dans le sélecteur), joignez-les directement — ils sont envoyés au modèle
  (vision, transcription native). Sinon, un message vous invite à changer
  de modèle ; les types acceptés par la zone de saisie suivent
  automatiquement le modèle courant.
- **Réponses citées** : les sources sont citées avec liens cliquables.
- **Feedback** : 👍/👎 sur une réponse — aidez vos admins à améliorer les agents.
- **Régénérer / éditer** : modifiez votre message et relancez la réponse.
- **Messages en file** : pendant une réponse en cours, tapez la question
  suivante — elle part automatiquement à la fin (max 5).

### Recherche dans vos documents
- Les réponses s'appuient sur l'index de recherche (keyword + vectoriel).
- Utilisez des filtres de sources (connecteurs, type de document).

---

## 5. Agents (assistants)

Un **agent** = un assistant spécialisé avec son prompt, ses outils et ses
documents de référence.

- **Agents par défaut** : l'« Lumen Assistant » répond à tout avec accès à
  l'ensemble des connaissances.
- **Créer un agent** : `Explore Agents` → `Nouveau`.
  - Nom, description, **prompt système** (son rôle, son ton).
  - **Outils** activables : recherche interne, recherche web, lecture de
    fichiers, mémoire utilisateur, génération d'images, URL ouverte, outils
    MCP/OpenAPI définis par l'admin.
  - **Documents de référence** : jeux de documents (Document Sets) et
    fichiers de projet dédiés.
  - **Partage** : public pour tous, ou restreint à des **groupes** d'utilisateurs.
- **Projets** : regroupez fichiers + chats autour d'un sujet ; les fichiers de
  projet sont injectés automatiquement dans les chats du projet.

---

## 6. Ce que l'admin peut faire (Admin Panel)

> Accès : menu **Admin** (réservé aux admins). Vue d'ensemble par section.

### 6.1 Agents & connaissances
- **Agents** : créer/éditer/partager les agents, définir les outils.
- **Document Sets** : regrouper des documents (par connecteur ou upload) et
  les rendre visibles à des groupes.
- **Indexing** : état de l'indexation, planification des connecteurs.

### 6.2 Intégrations
- **Connecteurs** : Google Drive, Slack, Confluence, GitHub, SharePoint,
  Notion, Jira, sites web (web connector), fichiers… Configurer la source,
  les accès, la fréquence de synchronisation.
- **Slack / Discord bots** : un bot Lumen répond directement dans vos canaux.
- **MCP Actions** : connecter des serveurs MCP (outils externes pour les
  agents) — avec clé API, OAuth, ou auth par utilisateur.
- **OpenAPI Actions** : brancher n'importe quelle API REST comme outil.

### 6.3 Modèles IA
- **Language Models** : configurer les fournisseurs LLM — OpenAI, Anthropic,
  **Ollama (local)**, Azure, Bedrock, Vertex… et choisir le modèle par défaut
  (chat, vision, naming).
- **Catalogue models.dev** : dans les modales de provider (OpenAI-Compatible
  et Custom), le bouton **« Browse models.dev catalog »** liste 213+
  fournisseurs avec endpoint API, clés d'auth, documentation et **modalités
  d'entrée par modèle** (texte/image/audio/vidéo/PDF, contexte). Sélectionnez
  les modèles → ils sont ajoutés avec les bons flags (dont l'option
  « Multimodal » par modèle).
- **Web Search** : fournisseur de recherche internet (exigé par l'outil web).
- **Image Generation** : fournisseur d'images (activé sur demande).
- **Voice** : transcription (STT) et synthèse vocale (TTS) optionnelles.

### 6.4 Permissions & organisation
- **Groupes** : créer des groupes d'utilisateurs, leur attribuer des
  **permissions** (gérer les connecteurs, les agents, les LLM…), et partager
  des ressources (agents, document sets, connecteurs).
- **Users** : inviter des utilisateurs, promouvoir admin, éditer les groupes.
- **SSO Providers** : SAML / OIDC / Google — SSO d'entreprise.
- **Service Accounts** : clés API pour l'automatisation (API REST).

### 6.5 Apparence & sécurité
- **Theme / Branding** : nom de l'application, logo, logotype.
- **Security** : durcissement (protection SSRF, masquage des credentials…).
- **Chat Preferences** : comportement par défaut des chats.

### 6.6 Analytics
- **Workspace Analytics** : usage par période, rapports exportables.
- **Query History** : historique des requêtes (suivant configuration).

---

## 7. Configurer un LLM local (Ollama) — tutoriel admin

1. Installer Ollama sur le serveur : `curl -fsSL https://ollama.com/install | sh`
2. Charger un modèle : `ollama pull llama3.1:8b`
3. Admin Panel → **Language Models** → ajouter un provider :
   - Type : **Ollama** — URL : `http://host.docker.internal:11434`
   - (depuis une VM linux : `http://172.17.0.1:11434` ou l'IP du host)
4. Sélectionner les modèles détectés → définir le modèle par défaut.
5. (Optionnel, embeddings 100 % local) le model server intégré embarque déjà
   `nomic-embed-text` — aucun service externe requis.

💡 Astuce : la recherche web (tool) exige un fournisseur configuré dans
**Web Search** — sans ça, désactivez l'outil pour vos agents.

---

## 8. API & automatisation

- API REST complète sous `/api` (documents, chats, agents, connecteurs…).
- Clés de **Service Account** : Admin Panel → Service Accounts.
- **Widget** chat embarquable sur un site (`widget/`).
- **Bot Slack/Discord** : mentionnez le bot dans un canal pour discuter.

---

## 9. Questions fréquentes

**« Mes données sortent-elles ? »**
Non, sauf vers le LLM que l'admin a configuré. Avec Ollama local, tout reste
sur l'infrastructure de l'entreprise.

**« Un fichier reste en “Processing…” »**
L'indexation peut prendre 1–2 min (embedding). Si ça dure : vérifier l'état
des workers (voir guide technique, § Dépannage).

**« Je ne vois pas le bouton X dans l'admin »**
Les entrées de l'admin dépendent de vos **permissions** et des outils
configurés (ex. Image Generation n'apparaît qu'une fois un provider ajouté).

**« Comment changer le modèle de chat ? »**
Clic sur le sélecteur de modèle en bas de la barre de saisie (les agents
peuvent aussi forcer leur modèle).

**« Puis-je envoyer un fichier audio/vidéo ? »**
Oui si le modèle courant l'accepte (ex. modèle multimodal) : la zone de
saisie l'indique. Sinon, utilisez un modèle multimodal ou laissez l'admin
activer le flag dans la configuration du modèle.

**« Reset de mon mot de passe ? »**
Cliquer « Mot de passe oublié » — nécessite le SMTP configuré par l'admin.

---

## 10. Bonnes pratiques

1. **Un agent par cas d'usage** (support IT, RH, ventes…) plutôt qu'un agent
   fourre-tout : de meilleurs prompts = de meilleures réponses.
2. **Document Sets nommés par équipe/projet** et partagés aux bons groupes.
3. **Commencez local** (Ollama) pour les données sensibles, ajoutez un LLM
   cloud si besoin de plus de qualité.
4. **Purgez l'historique** via les réglages de rétention si conformité.
5. Formez les équipes sur le **drag & drop de fichiers** — c'est le gain
   de productivité le plus immédiat.
