# Genuine schema16 checkpoint

status=review
base=af6d7e89576c367ada46f6135c384b94cf1984b2
test_source_sha256=231590bcc2b58dbbc20a7cbbbd693417504ccef567c9761e73a3782d73d8d481
dirty_at_capture=only reserved Rust test and task documentation
baseline=8 pass, 2 fail, no ignored
verification=required target 11 pass, strict feature-target Clippy and workspace fmt pass
contracts=producer immutable and verified, original 55 tables and raw receipts retained, real Store19, both backups and empty Host, original committed-WAL rollback, separate wrong19/future20 preservation
retained_reads=canonical immutable read receipts and old raw logs archived
processes=all owned compiles and test processes joined
next=scope and exact preview, leader review; retain two reservations
