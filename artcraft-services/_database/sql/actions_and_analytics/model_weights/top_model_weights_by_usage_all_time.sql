
-- Find top model weights by usage.
-- Remove "AI Streamers" and top users
-- This supports old-format TTS tokens.
-- This is a very expensive query that takes forever to run:
-- 1000 rows in set (7 hours 6 min 8.98 sec)
SELECT
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at,
  count(*) as use_count
FROM model_weights as mw
JOIN (
  -- First subquery: media_files records that reference model_weights or migrated model_weights
  select
    coalesce(w.token, w_migrated.token) as token
  from media_files as f
  left outer join model_weights as w
     on f.maybe_origin_model_token = w.token
  left outer join model_weights as w_migrated
    on f.maybe_origin_model_token = w_migrated.maybe_migration_old_model_token
  where
    f.maybe_creator_user_token NOT IN (
      "U:03HG1NRMDT7A6", "U:0BJT98T611SZC", "U:0T2NJR5VJRT8Z", "U:19WD4DHDR8XDH", "U:1S4G4YY0EW2X6", "U:211ZHWM8J1F8B",
      "U:246RSQQGZE7F7", "U:2H5SED9TD85AW", "U:2RAPWS9K5HEYZ", "U:2TEZS9T6AK6CY", "U:36910S9YX5M7S", "U:3MQB45NCHSNH9",
      "U:3S8S9X3H9B6SA", "U:3T08BX8GFDGJ2", "U:4MFYA09TH7SD1", "U:4Z8DK9T7K4170", "U:5P0BDXD18ST8C", "U:5W2X30KK6KNAP",
      "U:5WTDZVMX23WC1", "U:5Z4ZMBVZT8FYP", "U:6JJJRNN0B17CP", "U:6SCFF1NBHARQA", "U:70C2DNQ8NAPE2", "U:7HV0JNV82F85J",
      "U:7N1GDKHSSSZNJ", "U:7WAD25W85TYHM", "U:89XRNEWENPCGX", "U:8D706H5C7E5MF", "U:9QH9CFSXE5EY8", "U:9T28V6PKPB7EM",
      "U:A9HWW4VAE6XJ5", "U:AF0K4PMS37JWN", "U:BY6920DKHNDBT", "U:DG228ZACF1NAN", "U:DX55CFAPANPK3", "U:E4TSGYT5DX19C",
      "U:ESVW6NHE6GHP8", "U:EW15EA1XNGFVB", "U:F1DXEJZ7HS137", "U:FVFZ0TN0Y0N4F", "U:FYNFQA4D40PH4", "U:GJJKBHK15C9W0",
      "U:GSQGDX7HCQ9DK", "U:H2KWH2QPNHDRB", "U:JV591Y7HGVKMR", "U:KEXZ6BBGH75AX", "U:MBNYW84PRT7BB", "U:N0VXP0H0E36WT",
      "U:NBNCMS81965ZS", "U:NFV8FYBZHZPP4", "U:P6W06DB29G9SZ", "U:PM36EZDVKCMW3", "U:QJ9PGBCJR2101", "U:QPEXYBZJ5TB9K",
      "U:QYGQSZ0232EXP", "U:R2MATFEE7223X", "U:RAE3M212X65W6", "U:RBH6ZHBDDVH3Q", "U:SVZRVJSP04JAE", "U:SYW8H845YZJBW",
      "U:T3EE4GCA5Q4ZJ", "U:T7T6XH9540GVV", "U:TBM1Z40A46NXZ", "U:V7VA6DS24T2VE", "U:VVFERXHKNT4KN", "U:VW9XKV9NWV6WS",
      "U:WA1Q15SB6Q1VJ", "U:WG9S2GHDW8MA0", "U:WHB70A279YBPJ", "U:WHXJK1RSYM5WD", "U:WJJT9AHJHGJS5", "U:WPF17KR1HB4YV",
      "U:XACATP3C7NG5D", "U:XWA2SDNQBYMXD", "U:Y2Q8Q7C537G88", "U:YASA2RC19EHGY", "U:YX31ZJ517T40G", "U:Z477H1KNSY3B5",
      "user_23jmwykxjjrm4", "user_2xes0wjhfmybb", "user_4c2qcpbq6twv7", "user_5eq7hj7vnmqqc", "user_73k9zn3jxvcxs",
      "user_7yjk723jtfmj6", "user_8h8ffwmgphmq9", "user_9rjpehtnpd6tb", "user_a7nef57s5ypgm", "user_as01gf1gfc8ny",
      "user_at41z2z9zafdz", "user_aw7m4d07fjzrv", "user_bkypxftv69675", "user_bz76arxrmr0vj", "user_cewjs06e36hfq",
      "user_dppfbvwjq2963", "user_e9fwd2xfn2ajv", "user_gv0a0vec80540", "user_hdav69agkm2mw", "user_kqfdj8rdhkpnv",
      "user_mhzm8s7q9jt6h", "user_q7dj0hj3xy31g", "user_qcnpkvw9fnhyr", "user_vb7mch05njgnp", "user_vmdq5nq7bfckh",
      "user_wb4592qst8t48", "user_wm3tzh0v91gnv", "user_x990jyt3ne4br", "user_xy3snt46r1edn", "user_y1b4ag72ezh65",
      "user_y5q4s4he1snq9", "user_zgw9q4tz6506t"
    )
    and (w.token IS NOT NULL OR w_migrated.token IS NOT NULL)

  UNION ALL

  -- Second subquery: tts_results that reference tts_models (which we have migrated to model_weights)
  select
    w_migrated_2.token as token
  from tts_results as tr
  left outer join model_weights as w_migrated_2
     on tr.model_token = w_migrated_2.maybe_migration_old_model_token
  where
    tr.maybe_creator_user_token NOT IN (
      "U:03HG1NRMDT7A6", "U:0BJT98T611SZC", "U:0T2NJR5VJRT8Z", "U:19WD4DHDR8XDH", "U:1S4G4YY0EW2X6", "U:211ZHWM8J1F8B",
      "U:246RSQQGZE7F7", "U:2H5SED9TD85AW", "U:2RAPWS9K5HEYZ", "U:2TEZS9T6AK6CY", "U:36910S9YX5M7S", "U:3MQB45NCHSNH9",
      "U:3S8S9X3H9B6SA", "U:3T08BX8GFDGJ2", "U:4MFYA09TH7SD1", "U:4Z8DK9T7K4170", "U:5P0BDXD18ST8C", "U:5W2X30KK6KNAP",
      "U:5WTDZVMX23WC1", "U:5Z4ZMBVZT8FYP", "U:6JJJRNN0B17CP", "U:6SCFF1NBHARQA", "U:70C2DNQ8NAPE2", "U:7HV0JNV82F85J",
      "U:7N1GDKHSSSZNJ", "U:7WAD25W85TYHM", "U:89XRNEWENPCGX", "U:8D706H5C7E5MF", "U:9QH9CFSXE5EY8", "U:9T28V6PKPB7EM",
      "U:A9HWW4VAE6XJ5", "U:AF0K4PMS37JWN", "U:BY6920DKHNDBT", "U:DG228ZACF1NAN", "U:DX55CFAPANPK3", "U:E4TSGYT5DX19C",
      "U:ESVW6NHE6GHP8", "U:EW15EA1XNGFVB", "U:F1DXEJZ7HS137", "U:FVFZ0TN0Y0N4F", "U:FYNFQA4D40PH4", "U:GJJKBHK15C9W0",
      "U:GSQGDX7HCQ9DK", "U:H2KWH2QPNHDRB", "U:JV591Y7HGVKMR", "U:KEXZ6BBGH75AX", "U:MBNYW84PRT7BB", "U:N0VXP0H0E36WT",
      "U:NBNCMS81965ZS", "U:NFV8FYBZHZPP4", "U:P6W06DB29G9SZ", "U:PM36EZDVKCMW3", "U:QJ9PGBCJR2101", "U:QPEXYBZJ5TB9K",
      "U:QYGQSZ0232EXP", "U:R2MATFEE7223X", "U:RAE3M212X65W6", "U:RBH6ZHBDDVH3Q", "U:SVZRVJSP04JAE", "U:SYW8H845YZJBW",
      "U:T3EE4GCA5Q4ZJ", "U:T7T6XH9540GVV", "U:TBM1Z40A46NXZ", "U:V7VA6DS24T2VE", "U:VVFERXHKNT4KN", "U:VW9XKV9NWV6WS",
      "U:WA1Q15SB6Q1VJ", "U:WG9S2GHDW8MA0", "U:WHB70A279YBPJ", "U:WHXJK1RSYM5WD", "U:WJJT9AHJHGJS5", "U:WPF17KR1HB4YV",
      "U:XACATP3C7NG5D", "U:XWA2SDNQBYMXD", "U:Y2Q8Q7C537G88", "U:YASA2RC19EHGY", "U:YX31ZJ517T40G", "U:Z477H1KNSY3B5",
      "user_23jmwykxjjrm4", "user_2xes0wjhfmybb", "user_4c2qcpbq6twv7", "user_5eq7hj7vnmqqc", "user_73k9zn3jxvcxs",
      "user_7yjk723jtfmj6", "user_8h8ffwmgphmq9", "user_9rjpehtnpd6tb", "user_a7nef57s5ypgm", "user_as01gf1gfc8ny",
      "user_at41z2z9zafdz", "user_aw7m4d07fjzrv", "user_bkypxftv69675", "user_bz76arxrmr0vj", "user_cewjs06e36hfq",
      "user_dppfbvwjq2963", "user_e9fwd2xfn2ajv", "user_gv0a0vec80540", "user_hdav69agkm2mw", "user_kqfdj8rdhkpnv",
      "user_mhzm8s7q9jt6h", "user_q7dj0hj3xy31g", "user_qcnpkvw9fnhyr", "user_vb7mch05njgnp", "user_vmdq5nq7bfckh",
      "user_wb4592qst8t48", "user_wm3tzh0v91gnv", "user_x990jyt3ne4br", "user_xy3snt46r1edn", "user_y1b4ag72ezh65",
      "user_y5q4s4he1snq9", "user_zgw9q4tz6506t"
    )
) as x
on mw.token = x.token
group by
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at
order by use_count desc
limit 1000;





-------- Only media_files version of the query -------

-- Find top model weights by usage.
-- Remove "AI Streamers" and top users
-- This supports old-format TTS tokens.
-- This only queries the media_files table and runs in just 8 minutes.
SELECT
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at,
  count(*) as use_count
FROM model_weights as mw
JOIN (
  select
    coalesce(w.token, w_migrated.token) as token
  from media_files as f
  left outer join model_weights as w
     on f.maybe_origin_model_token = w.token
  left outer join model_weights as w_migrated
    on f.maybe_origin_model_token = w_migrated.maybe_migration_old_model_token
  and f.maybe_creator_user_token NOT IN (
      "U:03HG1NRMDT7A6", "U:0BJT98T611SZC", "U:0T2NJR5VJRT8Z", "U:19WD4DHDR8XDH", "U:1S4G4YY0EW2X6", "U:211ZHWM8J1F8B",
      "U:246RSQQGZE7F7", "U:2H5SED9TD85AW", "U:2RAPWS9K5HEYZ", "U:2TEZS9T6AK6CY", "U:36910S9YX5M7S", "U:3MQB45NCHSNH9",
      "U:3S8S9X3H9B6SA", "U:3T08BX8GFDGJ2", "U:4MFYA09TH7SD1", "U:4Z8DK9T7K4170", "U:5P0BDXD18ST8C", "U:5W2X30KK6KNAP",
      "U:5WTDZVMX23WC1", "U:5Z4ZMBVZT8FYP", "U:6JJJRNN0B17CP", "U:6SCFF1NBHARQA", "U:70C2DNQ8NAPE2", "U:7HV0JNV82F85J",
      "U:7N1GDKHSSSZNJ", "U:7WAD25W85TYHM", "U:89XRNEWENPCGX", "U:8D706H5C7E5MF", "U:9QH9CFSXE5EY8", "U:9T28V6PKPB7EM",
      "U:A9HWW4VAE6XJ5", "U:AF0K4PMS37JWN", "U:BY6920DKHNDBT", "U:DG228ZACF1NAN", "U:DX55CFAPANPK3", "U:E4TSGYT5DX19C",
      "U:ESVW6NHE6GHP8", "U:EW15EA1XNGFVB", "U:F1DXEJZ7HS137", "U:FVFZ0TN0Y0N4F", "U:FYNFQA4D40PH4", "U:GJJKBHK15C9W0",
      "U:GSQGDX7HCQ9DK", "U:H2KWH2QPNHDRB", "U:JV591Y7HGVKMR", "U:KEXZ6BBGH75AX", "U:MBNYW84PRT7BB", "U:N0VXP0H0E36WT",
      "U:NBNCMS81965ZS", "U:NFV8FYBZHZPP4", "U:P6W06DB29G9SZ", "U:PM36EZDVKCMW3", "U:QJ9PGBCJR2101", "U:QPEXYBZJ5TB9K",
      "U:QYGQSZ0232EXP", "U:R2MATFEE7223X", "U:RAE3M212X65W6", "U:RBH6ZHBDDVH3Q", "U:SVZRVJSP04JAE", "U:SYW8H845YZJBW",
      "U:T3EE4GCA5Q4ZJ", "U:T7T6XH9540GVV", "U:TBM1Z40A46NXZ", "U:V7VA6DS24T2VE", "U:VVFERXHKNT4KN", "U:VW9XKV9NWV6WS",
      "U:WA1Q15SB6Q1VJ", "U:WG9S2GHDW8MA0", "U:WHB70A279YBPJ", "U:WHXJK1RSYM5WD", "U:WJJT9AHJHGJS5", "U:WPF17KR1HB4YV",
      "U:XACATP3C7NG5D", "U:XWA2SDNQBYMXD", "U:Y2Q8Q7C537G88", "U:YASA2RC19EHGY", "U:YX31ZJ517T40G", "U:Z477H1KNSY3B5",
      "user_23jmwykxjjrm4", "user_2xes0wjhfmybb", "user_4c2qcpbq6twv7", "user_5eq7hj7vnmqqc", "user_73k9zn3jxvcxs",
      "user_7yjk723jtfmj6", "user_8h8ffwmgphmq9", "user_9rjpehtnpd6tb", "user_a7nef57s5ypgm", "user_as01gf1gfc8ny",
      "user_at41z2z9zafdz", "user_aw7m4d07fjzrv", "user_bkypxftv69675", "user_bz76arxrmr0vj", "user_cewjs06e36hfq",
      "user_dppfbvwjq2963", "user_e9fwd2xfn2ajv", "user_gv0a0vec80540", "user_hdav69agkm2mw", "user_kqfdj8rdhkpnv",
      "user_mhzm8s7q9jt6h", "user_q7dj0hj3xy31g", "user_qcnpkvw9fnhyr", "user_vb7mch05njgnp", "user_vmdq5nq7bfckh",
      "user_wb4592qst8t48", "user_wm3tzh0v91gnv", "user_x990jyt3ne4br", "user_xy3snt46r1edn", "user_y1b4ag72ezh65",
      "user_y5q4s4he1snq9", "user_zgw9q4tz6506t"
  )
) as x
on mw.token = x.token
group by
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at
order by use_count desc
limit 100;


+----------------------------------+--------------+----------------------------------------------------------------------------------------------------------------+---------------------+---------------------+----------------+-----------+
| token                            | weights_type | title                                                                                                          | created_at          | user_deleted_at     | mod_deleted_at | use_count |
+----------------------------------+--------------+----------------------------------------------------------------------------------------------------------------+---------------------+---------------------+----------------+-----------+
| weight_0f762jdzgsy1dhpb86qxy4ssm | tt2          | Rick Sanchez (Version 2.0)                                                                                     | 2023-05-14 05:26:03 | NULL                | NULL           |    846105 |
| weight_sfyjyr67ag1647xs0r7gmvkks | tt2          | Deleted Model                                                                                                  | 2023-09-27 15:06:37 | NULL                | NULL           |    702990 |
| weight_kwmxssf6k6ha92h3b55mh3w9q | tt2          | Waldemaro Martínez. (Locutor de DJ, Latin American Spanish.)                                                   | 2022-12-22 02:59:57 | NULL                | NULL           |    426417 |
| weight_ahxbf2104ngsgyegncaefyy6j | tt2          | Plankton v2 (Doug Lawrence)                                                                                    | 2022-08-11 06:19:56 | NULL                | NULL           |    401748 |
| weight_rhfg4chgrp42bnha8kqfrtmcq | tt2          | Mariano Closs (full version)                                                                                   | 2022-04-28 20:57:04 | NULL                | NULL           |    377601 |
| weight_34vt3stah9xyts72zrya85vc9 | tt2          | "Arthur C. Clarke" (901ep)                                                                                     | 2022-04-15 08:34:03 | NULL                | NULL           |    297574 |
| weight_hehgvegadf08mfp5rzd69dmh4 | tt2          | Angry Male Yelling                                                                                             | 2023-10-20 11:30:46 | NULL                | NULL           |    291089 |
| weight_hz7g8f1j4psrsw2sv67e4y61q | tt2          | Mariano Closs (Relator de fútbol Argentino)                                                                    | 2022-04-11 01:45:01 | NULL                | NULL           |    264792 |
| weight_83bv8pnva9vewht8zpye1x060 | tt2          | Dragonball Z Narrador (Latin, Version 1.0)                                                                     | 2022-07-28 05:05:33 | NULL                | NULL           |    261753 |
| weight_txvtzmcd7jw0rg192284r1g3w | tt2          | Son Goku. (IMITADOR Remasterizado.) (Dragon Ball, Latin American Spanish.)                                     | 2023-10-07 20:10:36 | NULL                | NULL           |    252080 |
| weight_b8rncypy7gw6nb0wthnwe2kk4 | tt2          | Zelda (Breath of the Wild)                                                                                     | 2023-05-17 00:30:49 | NULL                | NULL           |    247036 |
| weight_h8ebh6fyjyrr1vsjregw6yz8y | tt2          | Eric Cartman (Angry)                                                                                           | 2023-08-18 22:17:34 | NULL                | NULL           |    220012 |
| weight_mbcr352wfb1eq76tpy3ef3kx1 | tt2          | Lionel Messi                                                                                                   | 2022-09-08 22:50:12 | NULL                | NULL           |    193128 |
| weight_31ewdsvev9bttgb4eg7zy7mj5 | tt2          | Morgan Freeman (New)                                                                                           | 2023-09-08 05:20:34 | NULL                | NULL           |    189755 |
| weight_t7a4br08b76btnnwkb32tsh7q | tt2          | Cristiano Ronaldo. (Español) Versión 2.                                                                        | 2023-10-05 15:12:04 | NULL                | NULL           |    180499 |
| weight_fte9rq3em7vv9rqex0f8cvwdx | tt2          | Mariano Closs Tono Alto (Relator Fox Sports, Espn)                                                             | 2022-07-16 02:09:31 | NULL                | NULL           |    177236 |
| weight_kksrk14y802808as3d6mphedm | tt2          | Wednesday Addams (Jenna Ortega)                                                                                | 2023-03-06 21:22:52 | NULL                | NULL           |    161520 |
| weight_zyadhwrc3bspetkxcma5300gs | tt2          | Peter Griffin (Classic, Version 2.0)                                                                           | 2023-01-26 05:24:39 | NULL                | NULL           |    150694 |
| weight_r95f5pvvstgps0ychfqrzpfs3 | tt2          | Dragonball Z Narrador (Latin, Versión 2.0)                                                                     | 2023-02-04 11:21:06 | NULL                | NULL           |    146141 |
| weight_3k28fws0v6r1ke3p0w0vw48gm | tt2          | Eric Cartman (New)                                                                                             | 2023-08-18 22:18:12 | NULL                | NULL           |    144175 |
| weight_0cg1294gaf52c7rh0vz7a2ger | tt2          | Stan Lee                                                                                                       | 2021-08-17 10:56:25 | NULL                | NULL           |    133422 |
| weight_81waz0re2yzgw2fnmj0ah94zc | tt2          | Leonel Messi (Español)                                                                                         | 2022-06-05 13:31:20 | NULL                | NULL           |    129950 |
| weight_jzs5qmvtbvk934680ghfp48cf | tt2          | Homero Simpson (Homer Simpson Latin American Spanish)                                                          | 2022-12-25 14:52:16 | NULL                | NULL           |    129133 |
| weight_35ts2r0y8zkxzka82qh2jtr9h | sd_1.5       | Clarity 3 (Best Base Model)                                                                                    | 2024-02-18 22:28:46 | NULL                | NULL           |    122394 |
| weight_0bsxzrwp6p77t67s8dcn607wf | tt2          | Walter White (New)                                                                                             | 2022-12-13 07:35:03 | NULL                | NULL           |    118258 |
| weight_47hcjvnyfff60g4vty4thp400 | tt2          | Bob Esponja (SpongeBob SquarePants, Latin American Spanish)                                                    | 2022-02-13 01:55:52 | NULL                | NULL           |    115011 |
| weight_9409hbznp60n6w7ey9f6qw5bh | tt2          | Jar Jar Binks                                                                                                  | 2023-05-26 22:08:02 | NULL                | NULL           |    105357 |
| weight_tq6pwerrbr4mvbjmtyhbsqe6t | tt2          | Spongebob SquarePants (New)                                                                                    | 2023-04-01 10:47:13 | NULL                | NULL           |     98595 |
| weight_x6r5w2tsxgcrrsgweva6dkrqj | tt2          | Donald Trump (Angry)                                                                                           | 2022-08-29 11:49:46 | NULL                | NULL           |     92875 |
| weight_62nh7prayvcnevbg2hq5qp1r8 | tt2          | Jesse Pinkman (New)                                                                                            | 2023-09-27 20:34:33 | NULL                | NULL           |     92259 |
| weight_f4av238kbadw9h258v1cq1mxw | tt2          | Saul Goodman                                                                                                   | 2022-09-10 13:30:24 | NULL                | NULL           |     84818 |
| weight_ren3ng1bvpktsvg58gcrv2s52 | tt2          | Cristiano Ronaldo                                                                                              | 2023-02-18 17:16:22 | NULL                | NULL           |     83957 |
| weight_qnn4ns4r2a8x5abgb6fdg7tyz | tt2          | Homer Simpson (New)                                                                                            | 2023-12-04 20:58:48 | NULL                | NULL           |     75059 |
| weight_nzmwdhg42r8m6raap258mb55p | tt2          | Meg Griffin (Mila Kunis)                                                                                       | 2021-12-12 21:05:49 | NULL                | NULL           |     73956 |
| weight_79nve5ntc03rd2k5532je1rf6 | tt2          | Fluttershy. (Remasterizada) (Latin American Spanish, My Little Pony.)                                          | 2023-10-23 23:43:11 | NULL                | NULL           |     73700 |
| weight_ms1kzt5m09cfw1yn666cxhy88 | tt2          | Gerry Scotti                                                                                                   | 2022-09-29 22:14:26 | NULL                | NULL           |     72964 |
| weight_s0zjjkmht28dp4gm83e6xvjdk | tt2          | Auronplay (época de YouTube)                                                                                   | 2022-06-14 15:59:00 | NULL                | NULL           |     72813 |
| weight_vbz0wq7wkv80casj3vfptrrpg | sd_1.5       | Aniverse V15 Pruned                                                                                            | 2024-02-29 19:49:52 | NULL                | NULL           |     69564 |
| weight_1f7xnr358bkrhtx8y0v9cbfr6 | tt2          | Lionel Messi. (Español 2020 - 2023.)                                                                           | 2023-09-28 20:02:33 | NULL                | NULL           |     69112 |
| weight_pttvmv1z6ppddypx37z1e8ptj | tt2          | Mike Ehrmantraut (New)                                                                                         | 2023-08-13 03:35:16 | NULL                | NULL           |     68980 |
| weight_79mn6535kkfey17bgpg9xty23 | tt2          | Doc Tops (Dr. Luca Merlini)                                                                                    | 2022-04-08 00:22:00 | NULL                | NULL           |     68632 |
| weight_vcvr8zmz7sa6byzsze7rbqfh6 | rvc_v2       | "Aquamarine" Aqua Hoshino (Oshi No Ko)                                                                         | 2023-12-18 13:26:55 | NULL                | NULL           |     68581 |
| weight_3dnyrwny7d4585pcfjdrghxsq | tt2          | Gustavo Fring (Giancarlo Esposito)                                                                             | 2022-10-14 21:20:08 | NULL                | NULL           |     68175 |
| weight_abe03ym9bztnzvx462kkc3fak | tt2          | Merlina Addams                                                                                                 | 2023-01-30 01:20:47 | NULL                | NULL           |     67616 |
| weight_wzp5nb29078wdnadg2aep4m8k | rvc_v2       | "Android No.21" [JPN - DBFZ]                                                                                   | 2023-10-23 04:25:53 | 2024-02-16 17:56:39 | NULL           |     65153 |
| weight_vrx7j407cxk45jenkrd769h9b | tt2          | Donald Trump (Casual Speech)                                                                                   | 2022-09-05 01:56:36 | NULL                | NULL           |     63223 |
| weight_y9arhnd7wjamezhqd27ksvmaz | tt2          | Squidward Tentacles                                                                                            | 2021-10-30 15:21:53 | NULL                | NULL           |     63061 |
| weight_s4bmmamst4a7ca2pg7tc52zwx | tt2          | Goku (Jose Antonio Gavira) (Castillian Spanish)                                                                | 2022-09-18 15:09:32 | NULL                | NULL           |     61928 |
| weight_bc0tjv6fs7c1ccrtw8mbwxay5 | tt2          | Sir David Attenborough (Version 2.0)                                                                           | 2023-07-05 13:43:53 | NULL                | NULL           |     61261 |
| weight_jephx9sjmwzc7t4x29jsw88jg | tt2          | Mike Rome                                                                                                      | 2023-08-19 08:55:25 | NULL                | NULL           |     59687 |
| weight_154man2fzg19nrtc15drner7t | tt2          | Patrick Star                                                                                                   | 2021-09-21 18:30:52 | NULL                | NULL           |     59674 |
| weight_cg6heneq0rmkas3qj7ccz0t0r | tt2          | Donald Trump (Version 3.0)                                                                                     | 2023-09-12 05:09:33 | NULL                | NULL           |     55991 |
| weight_yqexh77ntqyawzgh9fzash798 | sd_1.5       | Flat2D Animerge v3                                                                                             | 2024-02-28 21:00:43 | NULL                | NULL           |     54861 |
| weight_ytna28rq62as8t5zb0qtpf3p6 | tt2          | Homero Simpson Español Latino (Clasico)                                                                        | 2023-01-20 23:43:12 | NULL                | NULL           |     54697 |
| weight_x59j84hsy4zf8zj1rta0f2c0a | tt2          | Toretto (Fast and Furious, Latin American Spanish)                                                             | 2022-02-13 14:38:55 | NULL                | NULL           |     53181 |
| weight_m7ns3pee2hze7r5hs3c1c5ew3 | tt2          | Fernanfloo. (Modelo más expresivo.) (Latin American Spanish.)                                                  | 2023-01-02 00:16:04 | NULL                | NULL           |     51191 |
| weight_7nkrya6jq2gdmhvzvhwe6565a | tt2          | Baldi (Baldi Basics Education And Learning)                                                                    | 2022-10-07 13:37:12 | NULL                | NULL           |     49977 |
| weight_kfc9kqm18k4jc3q2j3acdzj1a | tt2          | Waldemaro Martienez (Locutor)                                                                                  | 2022-12-08 15:59:55 | NULL                | NULL           |     49856 |
| weight_828jq9waqx5g9dzp7gr6a2a1p | tt2          | Kanye West (V4)                                                                                                | 2022-10-09 00:18:56 | NULL                | NULL           |     48616 |


-- (WIP) Find top model weights by usage. This supports old-format TTS tokens.
SELECT
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at,
  count(*) as use_count
FROM model_weights as mw
JOIN (
  select
    coalesce(w.token, w_migrated.token) as token
  from media_files as f
  left outer join model_weights as w
     on f.maybe_origin_model_token = w.token
  left outer join model_weights as w_migrated
    on f.maybe_origin_model_token = w_migrated.maybe_migration_old_model_token
  where f.created_at > NOW() - INTERVAL 3 DAY
) as x
on mw.token = x.token
group by
  mw.token,
  mw.weights_type,
  mw.title,
  mw.created_at,
  mw.user_deleted_at,
  mw.mod_deleted_at
order by use_count desc
limit 100;


mysql> select count(*) from tts_results;
+-----------+
| count(*)  |
+-----------+
| 192830507 |  -- 192,830,507
+-----------+
1 row in set (8 min 47.22 sec)

mysql> select count(*) from media_files;
+----------+
| count(*) |
+----------+
| 40057845 |   -- 40,057,845 (sept 1st, 2024)
+----------+
1 row in set (36.59 sec)
