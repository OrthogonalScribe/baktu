# Why baktu Exists

As a "scratch your own itch" project, `baktu`'s *raison d'être* is indeed an itch for which existing scratchers were deemed unsatisfactory. Namely, backup and reorganization of a particular dataset with minimal metadata loss, while providing easy snapshot access and restoring. Given the goal of infinite retention, another factor was the ease of tool extension to support fast previewing of the snapshot to be created (c.f. the various possible `git` commit workflows). The dataset itself involved on the order of half a million files and 50,000 directories, and many duplicates of sensitive data (e.g. family photos) due to changing hierarchy and manual device backups in the past.

Various backup methods were found to be an insufficiently good fit for the evolving requirements:
* some block-level-deduplicating backup tools demonstrate concerning repository corruption issues. Regardless of the cause, this highlights the increased impact of data corruption with that deduplication approach. The benefits were considered insufficient for the use case, against the risks introduced
* snapshotting filesystems were considered somewhat risky due to having a complicated licensing situation or being less battle-tested, in addition to imposing more requirements on future readers of the backups
* `rsnapshot` and similar `rsync` setups provide a good local backup with a sufficiently simple and robust snapshot encoding, but each snapshot requires inodes proportional to the number of directories in the source dataset. This approach also lacks different-path deduplication and sufficient metadata recording, and needs separate tooling for integrity checking
    *  Apple's [Time Machine](https://en.wikipedia.org/wiki/Apple_Time_Machine) avoids this inode cost [via directory hard links](https://superuser.com/questions/588414/why-does-os-x-time-machine-create-directory-hardlinks), which is acceptable for a first-party tool in the more tightly controlled macOS ecosystem, but [not a good idea](https://askubuntu.com/questions/210741/why-are-hard-links-not-allowed-for-directories) in Linux
* more extensive metadata preservation (including file creation time, all extended attribute namespaces, inode flags) that works well in conjunction with data deduplication, is a niche requirement that is not reasonable to be expected to be supported by mainstream backup tools
    * some forms of metadata are even less likely to be available due to the speed of their adoption, for example file creation time:
        * 2010: [`xstat()` proposal](https://lwn.net/Articles/394298/)
        * 2016: [`xstat()` reintroduced as `statx()` patches](https://lwn.net/Articles/686106/)
        * 2017: [Linux 4.11 adds `statx(2)`](https://kernelnewbies.org/Linux_4.11#statx.282.29.2C_a_modern_stat.282.29_alternative)
        * 2018: [glibc adds `statx` support](https://www.phoronix.com/news/Glibc-2.28-Released)
        * 2019: [stat(1) uses `statx`](https://debbugs.gnu.org/cgi/bugreport.cgi?bug=14703#27)

Given all of the above, a custom backup tool was considered a feasible approach. This is helped by relaxation of many other usual requirements for backup tools:
* no networking - backup media is assumed to be locally accessible by the machine during snapshot creation
* no encryption - all involved storage media is presumed to use [FDE](https://en.wikipedia.org/wiki/Disk_encryption#Full_disk_encryption) if needed
* no snapshot rotation nor deletion - backed up data pruning is left as an exercise for the user (with future tools to ease the burden)
* less urgent need for backed up data protection via permissions and redundancy data, due to the single-user mostly-offline-storage use-case, and [3-2-1 backup workflow (de)](https://de.wikipedia.org/wiki/Datensicherung#3-2-1_Backup-Regel)
    * however, a future addition of a redundancy overlay would provide benefits, c.f. the [3-2-1-1-0 strategy (fr)](https://fr.wikipedia.org/wiki/Sauvegarde_(informatique)#Strat%C3%A9gie_3-2-1-1-0[2])
* and naturally, a single-user target audience allows the tool to minimize configurability, focus on a single happy path and make user errors less likely