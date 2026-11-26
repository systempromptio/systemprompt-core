-- Tables this member no longer owns. They are declared, not executed: the
-- install releases this member's record of each, and the member that now
-- declares the table takes it over with its rows.
--
-- content_files: the content–file links belong to the `content` member.
DROP TABLE IF EXISTS content_files;
