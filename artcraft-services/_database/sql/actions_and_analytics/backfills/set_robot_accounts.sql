-- Doc: https://docs.google.com/document/d/14AyEV_0PYwa71a5W8xRRZ8vbI0uZHQF4ycqi539qmAc/edit
-- backspace23: Twitch robot user? "Hello streamer! You look amazing today! Keep on being pog!." + usernames
-- blastbeng: Italian AI streamer? Only uses 23 voices, all Italian
-- bluemo69: (confirmed, abandoned) AI SpongeBob
-- dzth: AI streamer - 144,653 media files (all audio, mostly SpongeBob, using old tokens) in August 2024
-- idkman01: *likely* AI streamer for Sonic - 77,741 media files in August 2024, Sonic voices, some SpongeBob
-- johnloberger: (confirmed) AI South Park
-- peepostream: Twitch API tool (??), but long tail of voices
-- powerchat: Twitch API tool
-- rewritten_code: (confirmed) AI Breaking Bad
-- robertmctague: Definitely robot, but strange. Bimodal character set, auto-censor, bitcoin
-- thevisitorx: Only uses 32 voices. Very weird set of voices.
UPDATE users
SET is_api_user = TRUE
WHERE username IN (
  'backspace23',
  'blastbeng',
  'bluemo69',
  'dzth',
  'idkman01',
  'johnloberger',
  'leon_says',
  'peepostream',
  'powerchat',
  'rewritten_code',
  'robertmctague',
  'thevisitorx'
);

-- Ripping off our API (???):
-- We won't filter these from organic analysis because their usage is likely organic even if they're hijacking us
-- devproject2023 - lots of different voices used, in different languages
-- leon_says: 101soundboards again, lots of different voices used, in different languages
-- thebigo - not API hacker, but not paying us, using us at volume, making bad content??
-- yigithan - lots of different voices used, in different languages

-- Top August users:

+--------------------+-----------------+-------------+---------------------+
| user_token         | username        | usage_count | created_at          |
+--------------------+-----------------+-------------+---------------------+
| user_aw7m4d07fjzrv | robertmctague   |      286029 | 2024-03-18 21:51:42 | -- Strange. Bimodal character set, auto-censor, bitcoin
| U:03HG1NRMDT7A6    | leon_says       |      209682 | 2023-09-13 16:27:40 | -- 101soundboards - ripping off our API again
| U:Z477H1KNSY3B5    | dzth            |      145174 | 2023-07-31 15:53:50 |  AI SpongeBob
| user_vb7mch05njgnp | idkman01        |       77741 | 2024-06-29 03:33:34 |  AI Sonic + a little AI SpongeBob
| U:8D706H5C7E5MF    | yigithan        |       41965 | 2023-05-03 07:35:43 | -- Ripping off our API (???)
| U:4Z8DK9T7K4170    | devproject2023  |       29530 | 2023-03-05 21:19:27 | -- Ripping off our API (???)
| U:5Z4ZMBVZT8FYP    | blastbeng       |       21126 | 2022-10-25 21:33:29 | -- Italian AI streamer? Only uses 23 voices, all Italian
| U:YX31ZJ517T40G    | johnloberger    |       18340 | 2023-06-14 21:34:52 | AI South Park
| U:6JJJRNN0B17CP    | backspace23     |       16229 | 2023-10-15 07:57:08 | -- Twitch tool? "Hello streamer! You look amazing today! Keep on being pog!." + usernames
| U:89XRNEWENPCGX    | thevisitorx     |       15138 | 2022-12-16 09:44:37 | -- Only uses 32 voices. Very weird set of voices.
| U:QPEXYBZJ5TB9K    | thebigo         |       14141 | 2022-09-06 01:38:52 | -- 92 voices, almost exclusively female voices!

-- Top Users All Time

+------------------+--------------------+---------------------+-------------+
| username         | token              | created_at          | generations |
+------------------+--------------------+---------------------+-------------+
| leon_says        | U:03HG1NRMDT7A6    | 2023-09-13 16:27:40 |     1608442 |
| dzth             | U:Z477H1KNSY3B5    | 2023-07-31 15:53:50 |     1287340 |
| robertmctague    | user_aw7m4d07fjzrv | 2024-03-18 21:51:42 |      785684 |
| rewritten_code   | U:TBM1Z40A46NXZ    | 2023-07-18 00:46:12 |      633926 |
| devproject2023   | U:4Z8DK9T7K4170    | 2023-03-05 21:19:27 |      476896 |
| yigithan         | U:8D706H5C7E5MF    | 2023-05-03 07:35:43 |      368559 |
| johnloberger     | U:YX31ZJ517T40G    | 2023-06-14 21:34:52 |      354966 |
| blastbeng        | U:5Z4ZMBVZT8FYP    | 2022-10-25 21:33:29 |      226726 |
| backspace23      | U:6JJJRNN0B17CP    | 2023-10-15 07:57:08 |      212895 |
| nhapcs           | U:7HV0JNV82F85J    | 2023-07-12 15:09:07 |      182513 |
| thevisitorx      | U:89XRNEWENPCGX    | 2022-12-16 09:44:37 |      160763 |
| evanschris       | U:3T08BX8GFDGJ2    | 2023-08-30 07:14:44 |      130251 |
| idkman01         | user_vb7mch05njgnp | 2024-06-29 03:33:34 |       99008 |
| thebigo          | U:QPEXYBZJ5TB9K    | 2022-09-06 01:38:52 |       88822 |
| beatsolo         | U:SVZRVJSP04JAE    | 2023-09-08 03:21:51 |       88089 |
| banana12134      | user_xy3snt46r1edn | 2024-06-06 02:00:10 |       82637 |
| dwdwd            | U:BY6920DKHNDBT    | 2023-02-06 02:22:07 |       82488 |
| bluemo69         | U:4MFYA09TH7SD1    | 2023-06-19 22:12:11 |       76651 |
| catmem           | user_q7dj0hj3xy31g | 2024-05-02 09:57:28 |       49235 |
| bfdi_ceo         | user_hdav69agkm2mw | 2024-05-20 18:00:46 |       46563 |
| ahiman           | user_vmdq5nq7bfckh | 2024-04-13 05:47:40 |       37340 |
| kamalmango       | U:NFV8FYBZHZPP4    | 2023-07-31 19:12:30 |       35873 |
| brandon2000      | U:AF0K4PMS37JWN    | 2023-07-26 03:36:14 |       24899 |
| powerchat        | U:Y2Q8Q7C537G88    | 2023-01-23 23:51:37 |       24557 |
| storysonic17     | U:5W2X30KK6KNAP    | 2023-03-16 01:10:18 |       23561 |
| hexzd            | U:2H5SED9TD85AW    | 2023-08-28 02:18:15 |       21521 |
| gdinwtfdyd66692  | U:3S8S9X3H9B6SA    | 2022-10-12 16:50:25 |       20966 |
| thewildwolf      | user_7yjk723jtfmj6 | 2023-11-13 12:32:58 |       20858 |
| uberwaffle       | user_zgw9q4tz6506t | 2024-01-18 03:09:16 |       20856 |
| peepostream      | U:SYW8H845YZJBW    | 2023-07-12 20:37:26 |       20417 |
| cangqu           | user_x990jyt3ne4br | 2024-04-09 14:00:47 |       20260 |
| mostafa88t       | U:N0VXP0H0E36WT    | 2023-04-30 04:51:39 |       20107 |
| nicktea          | user_4c2qcpbq6twv7 | 2024-04-22 22:09:05 |       19322 |
| freddyj          | U:5WTDZVMX23WC1    | 2023-03-10 13:02:09 |       18084 |
| mikpas           | U:KEXZ6BBGH75AX    | 2023-08-07 23:32:30 |       16747 |
| cinnamonmick     | user_23jmwykxjjrm4 | 2024-01-26 18:27:56 |       14632 |
| jammythegranny   | user_bz76arxrmr0vj | 2023-12-30 15:21:04 |       14580 |
| colerw2001       | U:WJJT9AHJHGJS5    | 2023-07-25 01:56:49 |       14478 |
| saidulrifat      | U:GJJKBHK15C9W0    | 2023-10-01 15:41:29 |       14446 |
| cmboy            | U:E4TSGYT5DX19C    | 2022-06-19 23:33:33 |       14116 |
| drunkrussian     | U:ESVW6NHE6GHP8    | 2023-03-08 03:09:08 |       14037 |
| trag             | U:WHXJK1RSYM5WD    | 2023-06-15 16:54:33 |       13369 |
| coleroywarren    | U:3MQB45NCHSNH9    | 2022-12-25 00:17:22 |       13171 |
| havewelost       | U:2TEZS9T6AK6CY    | 2023-06-08 16:35:16 |       12953 |
| wawoul           | user_gv0a0vec80540 | 2024-01-15 16:19:10 |       12617 |
| nininoni3333     | user_bkypxftv69675 | 2024-02-16 01:26:30 |       12227 |
| bellacraft       | user_9rjpehtnpd6tb | 2024-03-19 08:41:13 |       12068 |
| knightrider0     | U:H2KWH2QPNHDRB    | 2023-08-31 17:37:48 |       11965 |
| itzultrascout    | U:EW15EA1XNGFVB    | 2022-05-12 17:12:48 |       11950 |
| citruskat        | user_qcnpkvw9fnhyr | 2024-04-04 03:57:18 |       11067 |
| rannyphant       | user_y1b4ag72ezh65 | 2024-01-29 23:59:00 |       10928 |
| xkendy           | user_cewjs06e36hfq | 2023-11-10 19:44:59 |       10790 |
| alexcharles      | U:19WD4DHDR8XDH    | 2023-09-18 18:45:02 |       10705 |
| jtoddd           | U:0BJT98T611SZC    | 2023-10-06 04:15:53 |       10420 |
| davisher         | U:T3EE4GCA5Q4ZJ    | 2022-11-22 20:39:40 |       10243 |
| moon1201         | U:211ZHWM8J1F8B    | 2023-10-05 07:43:03 |       10187 |

-- Continued (>5k to <10k):

| porkai           | U:RBH6ZHBDDVH3Q    | 2023-06-15 02:34:39 |        9703 |
| twitchguru       | U:GSQGDX7HCQ9DK    | 2023-08-03 14:47:40 |        9634 |
| ricardo73        | U:T7T6XH9540GVV    | 2023-04-24 06:25:31 |        9563 |
| inspired17       | user_as01gf1gfc8ny | 2024-03-02 04:03:28 |        9185 |
| july1337         | user_wb4592qst8t48 | 2024-04-05 15:58:28 |        8961 |
| ren51            | U:XWA2SDNQBYMXD    | 2023-09-07 14:40:00 |        8847 |
| hillbilly        | U:7N1GDKHSSSZNJ    | 2022-11-28 12:56:26 |        8645 |
| xdliverblx       | user_kqfdj8rdhkpnv | 2023-12-30 20:17:46 |        8608 |
| chocolateimage   | U:WHB70A279YBPJ    | 2023-07-21 15:53:52 |        8227 |
| nothingster      | U:A9HWW4VAE6XJ5    | 2022-12-20 02:51:05 |        8182 |
| bigboyduelist    | U:QYGQSZ0232EXP    | 2022-08-22 16:29:21 |        7962 |
| ibrasal          | U:MBNYW84PRT7BB    | 2023-08-16 20:25:34 |        7501 |
| nathanjackson    | user_dppfbvwjq2963 | 2023-11-17 21:37:48 |        7472 |
| tcw              | user_2xes0wjhfmybb | 2023-11-07 03:42:35 |        7416 |
| jpb1976          | U:P6W06DB29G9SZ    | 2023-02-04 15:10:08 |        7403 |
| mezayt           | U:0T2NJR5VJRT8Z    | 2022-12-28 05:12:49 |        7389 |
| jctdaswfmnqaee8  | user_mhzm8s7q9jt6h | 2023-12-31 00:17:41 |        7328 |
| icerogue         | user_73k9zn3jxvcxs | 2024-02-22 14:53:57 |        7258 |
| thelooneygirl    | user_e9fwd2xfn2ajv | 2023-11-30 17:15:53 |        7163 |
| not_popee        | U:WA1Q15SB6Q1VJ    | 2023-04-11 09:18:52 |        7068 |
| altazio          | U:6SCFF1NBHARQA    | 2022-09-08 09:39:59 |        6967 |
| virtualsignal    | user_y5q4s4he1snq9 | 2024-02-06 06:53:17 |        6867 |
| gioboss          | U:PM36EZDVKCMW3    | 2023-05-23 19:17:38 |        6759 |
| lucasday         | U:2RAPWS9K5HEYZ    | 2022-11-03 05:15:53 |        6754 |
| nickviv          | user_at41z2z9zafdz | 2024-07-08 17:52:51 |        6706 |
| novicain         | U:DG228ZACF1NAN    | 2022-12-28 01:49:00 |        6429 |
| n8kasper         | U:VW9XKV9NWV6WS    | 2022-05-31 21:25:56 |        6373 |
| garrett65536     | U:9T28V6PKPB7EM    | 2023-08-28 19:16:21 |        6366 |
| oklahomadoss     | U:WPF17KR1HB4YV    | 2023-01-23 01:31:37 |        6364 |
| rqguy2022        | U:JV591Y7HGVKMR    | 2022-09-08 22:35:59 |        6363 |
| jack314k         | U:FYNFQA4D40PH4    | 2023-06-10 02:29:41 |        6286 |
| michael-studios  | U:5P0BDXD18ST8C    | 2023-07-29 20:46:32 |        6276 |
| ill-greed        | U:YASA2RC19EHGY    | 2023-07-22 06:06:22 |        6170 |
| jamescozart2245  | U:V7VA6DS24T2VE    | 2023-07-25 01:17:30 |        6063 |
| avaljml          | U:F1DXEJZ7HS137    | 2023-08-22 02:51:09 |        6012 |
| 24ratcma         | U:NBNCMS81965ZS    | 2022-11-28 01:10:53 |        5957 |
| solitaryvirus    | user_wm3tzh0v91gnv | 2024-06-28 16:35:50 |        5942 |
| username64       | U:R2MATFEE7223X    | 2023-07-25 02:06:41 |        5879 |
| tellstories      | user_5eq7hj7vnmqqc | 2024-03-28 18:57:57 |        5824 |
| mallerie7        | U:7WAD25W85TYHM    | 2022-12-16 19:22:18 |        5777 |
| unknown123       | U:1S4G4YY0EW2X6    | 2022-03-08 02:03:25 |        5736 |
| truebuck86       | U:RAE3M212X65W6    | 2023-09-13 21:47:28 |        5689 |
| g4bdou1          | U:FVFZ0TN0Y0N4F    | 2023-04-30 23:40:31 |        5581 |
| hspillerga       | user_8h8ffwmgphmq9 | 2023-11-19 16:07:20 |        5562 |
| garflied17       | user_a7nef57s5ypgm | 2023-12-09 18:14:16 |        5432 |
| botwoon          | U:DX55CFAPANPK3    | 2023-03-12 05:23:03 |        5403 |
| jeromec22        | U:WG9S2GHDW8MA0    | 2023-07-21 23:54:37 |        5390 |
| forrealuseless   | U:VVFERXHKNT4KN    | 2023-05-07 23:13:07 |        5324 |
| xc3jo            | U:246RSQQGZE7F7    | 2022-09-12 03:48:02 |        5211 |
| codebullet       | U:9QH9CFSXE5EY8    | 2023-07-27 06:12:20 |        5184 |
| ziad1818         | U:XACATP3C7NG5D    | 2023-07-23 20:45:02 |        5158 |
| weslleycebr      | U:QJ9PGBCJR2101    | 2023-07-10 16:35:02 |        5143 |
| cristianarbey    | U:36910S9YX5M7S    | 2022-12-13 16:52:07 |        5128 |
| thiccslayer      | U:70C2DNQ8NAPE2    | 2022-07-21 04:29:47 |        5071 |