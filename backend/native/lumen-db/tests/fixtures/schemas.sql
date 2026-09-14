-- Fixtures for tests/tenant_scoping.rs.
--
-- Three schemas holding a same-named table with different rows, so a query that
-- resolves to the wrong schema returns the wrong owner rather than an error.
-- "tenant-b" carries a dash on purpose: it is legal in a tenant id and illegal
-- as a bare SQL identifier, so it is what proves the quoting works.

DROP SCHEMA IF EXISTS tenant_a CASCADE;
DROP SCHEMA IF EXISTS "tenant-b" CASCADE;
DROP TABLE IF EXISTS public.widget;

CREATE SCHEMA tenant_a;
CREATE SCHEMA "tenant-b";

CREATE TABLE tenant_a.widget   (id int primary key, owner text);
CREATE TABLE "tenant-b".widget (id int primary key, owner text);
CREATE TABLE public.widget     (id int primary key, owner text);

INSERT INTO tenant_a.widget   VALUES (1, 'belongs-to-a');
INSERT INTO "tenant-b".widget VALUES (1, 'belongs-to-b');
INSERT INTO public.widget     VALUES (1, 'belongs-to-public');
