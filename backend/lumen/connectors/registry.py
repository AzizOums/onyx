"""Registry mapping for connector classes."""

from pydantic import BaseModel

from lumen.configs.constants import DocumentSource


class ConnectorMapping(BaseModel):
    module_path: str
    class_name: str


# Mapping of DocumentSource to connector details for lazy loading
CONNECTOR_CLASS_MAP = {
    DocumentSource.WEB: ConnectorMapping(
        module_path="lumen.connectors.web.connector",
        class_name="WebConnector",
    ),
    DocumentSource.FILE: ConnectorMapping(
        module_path="lumen.connectors.file.connector",
        class_name="LocalFileConnector",
    ),
    DocumentSource.SLACK: ConnectorMapping(
        module_path="lumen.connectors.slack.connector",
        class_name="SlackConnector",
    ),
    DocumentSource.GITHUB: ConnectorMapping(
        module_path="lumen.connectors.github.connector",
        class_name="GithubConnector",
    ),
    DocumentSource.GMAIL: ConnectorMapping(
        module_path="lumen.connectors.gmail.connector",
        class_name="GmailConnector",
    ),
    DocumentSource.GITLAB: ConnectorMapping(
        module_path="lumen.connectors.gitlab.connector",
        class_name="GitlabConnector",
    ),
    DocumentSource.GITBOOK: ConnectorMapping(
        module_path="lumen.connectors.gitbook.connector",
        class_name="GitbookConnector",
    ),
    DocumentSource.GOOGLE_DRIVE: ConnectorMapping(
        module_path="lumen.connectors.google_drive.connector",
        class_name="GoogleDriveConnector",
    ),
    DocumentSource.BOOKSTACK: ConnectorMapping(
        module_path="lumen.connectors.bookstack.connector",
        class_name="BookstackConnector",
    ),
    DocumentSource.OUTLINE: ConnectorMapping(
        module_path="lumen.connectors.outline.connector",
        class_name="OutlineConnector",
    ),
    DocumentSource.CONFLUENCE: ConnectorMapping(
        module_path="lumen.connectors.confluence.connector",
        class_name="ConfluenceConnector",
    ),
    DocumentSource.JIRA: ConnectorMapping(
        module_path="lumen.connectors.jira.connector",
        class_name="JiraConnector",
    ),
    DocumentSource.PRODUCTBOARD: ConnectorMapping(
        module_path="lumen.connectors.productboard.connector",
        class_name="ProductboardConnector",
    ),
    DocumentSource.SLAB: ConnectorMapping(
        module_path="lumen.connectors.slab.connector",
        class_name="SlabConnector",
    ),
    DocumentSource.CODA: ConnectorMapping(
        module_path="lumen.connectors.coda.connector",
        class_name="CodaConnector",
    ),
    DocumentSource.CANVAS: ConnectorMapping(
        module_path="lumen.connectors.canvas.connector",
        class_name="CanvasConnector",
    ),
    DocumentSource.NOTION: ConnectorMapping(
        module_path="lumen.connectors.notion.connector",
        class_name="NotionConnector",
    ),
    DocumentSource.ZULIP: ConnectorMapping(
        module_path="lumen.connectors.zulip.connector",
        class_name="ZulipConnector",
    ),
    DocumentSource.GURU: ConnectorMapping(
        module_path="lumen.connectors.guru.connector",
        class_name="GuruConnector",
    ),
    DocumentSource.LINEAR: ConnectorMapping(
        module_path="lumen.connectors.linear.connector",
        class_name="LinearConnector",
    ),
    DocumentSource.HUBSPOT: ConnectorMapping(
        module_path="lumen.connectors.hubspot.connector",
        class_name="HubSpotConnector",
    ),
    DocumentSource.DOCUMENT360: ConnectorMapping(
        module_path="lumen.connectors.document360.connector",
        class_name="Document360Connector",
    ),
    DocumentSource.GONG: ConnectorMapping(
        module_path="lumen.connectors.gong.connector",
        class_name="GongConnector",
    ),
    DocumentSource.GOOGLE_SITES: ConnectorMapping(
        module_path="lumen.connectors.google_site.connector",
        class_name="GoogleSitesConnector",
    ),
    DocumentSource.ZENDESK: ConnectorMapping(
        module_path="lumen.connectors.zendesk.connector",
        class_name="ZendeskConnector",
    ),
    DocumentSource.LOOPIO: ConnectorMapping(
        module_path="lumen.connectors.loopio.connector",
        class_name="LoopioConnector",
    ),
    DocumentSource.BOX: ConnectorMapping(
        module_path="lumen.connectors.box.connector",
        class_name="BoxConnector",
    ),
    DocumentSource.DROPBOX: ConnectorMapping(
        module_path="lumen.connectors.dropbox.connector",
        class_name="DropboxConnector",
    ),
    DocumentSource.SHAREPOINT: ConnectorMapping(
        module_path="lumen.connectors.sharepoint.connector",
        class_name="SharepointConnector",
    ),
    DocumentSource.TEAMS: ConnectorMapping(
        module_path="lumen.connectors.teams.connector",
        class_name="TeamsConnector",
    ),
    DocumentSource.SALESFORCE: ConnectorMapping(
        module_path="lumen.connectors.salesforce.connector",
        class_name="SalesforceConnector",
    ),
    DocumentSource.DISCOURSE: ConnectorMapping(
        module_path="lumen.connectors.discourse.connector",
        class_name="DiscourseConnector",
    ),
    DocumentSource.AXERO: ConnectorMapping(
        module_path="lumen.connectors.axero.connector",
        class_name="AxeroConnector",
    ),
    DocumentSource.CLICKUP: ConnectorMapping(
        module_path="lumen.connectors.clickup.connector",
        class_name="ClickupConnector",
    ),
    DocumentSource.MEDIAWIKI: ConnectorMapping(
        module_path="lumen.connectors.mediawiki.wiki",
        class_name="MediaWikiConnector",
    ),
    DocumentSource.WIKIPEDIA: ConnectorMapping(
        module_path="lumen.connectors.wikipedia.connector",
        class_name="WikipediaConnector",
    ),
    DocumentSource.ASANA: ConnectorMapping(
        module_path="lumen.connectors.asana.connector",
        class_name="AsanaConnector",
    ),
    DocumentSource.S3: ConnectorMapping(
        module_path="lumen.connectors.blob.connector",
        class_name="BlobStorageConnector",
    ),
    DocumentSource.R2: ConnectorMapping(
        module_path="lumen.connectors.blob.connector",
        class_name="BlobStorageConnector",
    ),
    DocumentSource.GOOGLE_CLOUD_STORAGE: ConnectorMapping(
        module_path="lumen.connectors.blob.connector",
        class_name="BlobStorageConnector",
    ),
    DocumentSource.OCI_STORAGE: ConnectorMapping(
        module_path="lumen.connectors.blob.connector",
        class_name="BlobStorageConnector",
    ),
    DocumentSource.XENFORO: ConnectorMapping(
        module_path="lumen.connectors.xenforo.connector",
        class_name="XenforoConnector",
    ),
    DocumentSource.DISCORD: ConnectorMapping(
        module_path="lumen.connectors.discord.connector",
        class_name="DiscordConnector",
    ),
    DocumentSource.FRESHDESK: ConnectorMapping(
        module_path="lumen.connectors.freshdesk.connector",
        class_name="FreshdeskConnector",
    ),
    DocumentSource.FIREFLIES: ConnectorMapping(
        module_path="lumen.connectors.fireflies.connector",
        class_name="FirefliesConnector",
    ),
    DocumentSource.EGNYTE: ConnectorMapping(
        module_path="lumen.connectors.egnyte.connector",
        class_name="EgnyteConnector",
    ),
    DocumentSource.AIRTABLE: ConnectorMapping(
        module_path="lumen.connectors.airtable.airtable_connector",
        class_name="AirtableConnector",
    ),
    DocumentSource.HIGHSPOT: ConnectorMapping(
        module_path="lumen.connectors.highspot.connector",
        class_name="HighspotConnector",
    ),
    DocumentSource.DRUPAL_WIKI: ConnectorMapping(
        module_path="lumen.connectors.drupal_wiki.connector",
        class_name="DrupalWikiConnector",
    ),
    DocumentSource.IMAP: ConnectorMapping(
        module_path="lumen.connectors.imap.connector",
        class_name="ImapConnector",
    ),
    DocumentSource.BITBUCKET: ConnectorMapping(
        module_path="lumen.connectors.bitbucket.connector",
        class_name="BitbucketConnector",
    ),
    DocumentSource.TESTRAIL: ConnectorMapping(
        module_path="lumen.connectors.testrail.connector",
        class_name="TestRailConnector",
    ),
    DocumentSource.BRAINTRUST: ConnectorMapping(
        module_path="lumen.connectors.braintrust.connector",
        class_name="BraintrustConnector",
    ),
    DocumentSource.LUMAPPS: ConnectorMapping(
        module_path="lumen.connectors.lumapps.connector",
        class_name="LumAppsConnector",
    ),
    # just for integration tests
    DocumentSource.MOCK_CONNECTOR: ConnectorMapping(
        module_path="lumen.connectors.mock_connector.connector",
        class_name="MockConnector",
    ),
}
