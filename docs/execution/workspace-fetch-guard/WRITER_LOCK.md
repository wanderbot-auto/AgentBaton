# Writer-lock lifetime correction

Only core/ipc.rs changes at93ca7ea, onguardbranch32ca76. NativeARM realrestart failedWORKSPACE_BUSY, causationunproven. fs2 0.4.3 Unix usesflock onopenfiledescriptions; File::try_clone preservesdescription, so dropping oneFD alone neednotrelease flock. Same deterministiccontrol actuallyfails oldcode andpasses explicitunlock.

DaemonDrop unlinks ownsocket beforeunlock so a newowner's socket cannotbe removed bylateoldcleanup. Explicitunlock releasesonly oldownership; closedoldduplicate doesnotunlock newindependentdescription. Activewriter refusal remainsstrict. No sleep/retry/exception altered. No productionDaemonDrop can runconcurrentlywith its own by-value dispatchloop onthisbaseline; fields destruct afterward.

Thisbranch has noownedSupervisor worker. Whenporting toanactorvariant, ensure allownedworkers shutdown/reap beforeunlock onnormal,error andunwindpaths. Merelyplacingunlock beforeSupervisorfield destructor would notsatisfy lifecycleordering. Do not widen thisoldbaseline review toclaim worker cleanup.

ActualMacfailure rawlog andlocal oldFAIL/newPASS control, full131/Core36/projectIPC3/Clippy/fmt hashes are in EVIDENCE. Independent /root/review exactsource review and all36Coretests PASS approved this oldbaseline; exactcorrectednativeCI remainsneeded. Actorport requires separate teardownreview. No liveProvider/account/network/model execution.
