-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Remove temporary test models
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'temporary test model'
where token IN (
    'vcm_jt9mdz9xjjcb',
    'vcm_4xzkzdhs4kk5',
    'vcm_502wkt2pca3y'
);

-- Re-home Nessu's models
update voice_conversion_models set creator_user_token = "user_7z1h6sykg4r62" where creator_user_token = "U:C00NRSW1AE8T4" limit 5;

-- Remove broken models
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'broken model'
where token IN (
    'vcm_982y9g6v3jrg',
    'vcm_16rpyehf7f0a',
    'vcm_8vjtz3dbnw17',
    'vcm_tc9mwr60cjth',
    'vcm_bshsyp01w9vg',
    'vcm_31whj8apmqj7',
    'vcm_84qa0an44p8p',
    'vcm_9xx4v5qz5fbg',
    'vcm_7zp2b2hs2at2',
    'vcm_zhvk0mnnb7zp',
    'vcm_1vc4scq9qp6m',
    'vcm_sr1b54mwy0ym',
    'vcm_qx96y8aasydk'
);

-- Remove duplicate models
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'duplicate model'
where token IN (
    'vcm_6w21em1bs7q3',
    'vcm_sbhvyj7926w4',
    'vcm_anv469g89adz',
    'vcm_xsx5yx585c7x',
    'vcm_dpahzjjpkf18',
    'vcm_y09dqe6zgwyk',
    'vcm_xrp96dwgppv2',
    'vcm_84awjbjxts6n',
    'vcm_b2x2sn8kf28j',
    'vcm_41nw6xgr5x3v',
    'vcm_a6x83tk0ttf2',
    'vcm_dgjgqxq5w97j',
    'vcm_59k5j452x7xy',
    'vcm_09p6vqnk7n05',
    'vcm_attanwh383hg',
    'vcm_t8d4f6w9pa8t',
    'vcm_s8s4q4c65xsy',
    'vcm_6e2av8pmkyz5',
    'vcm_82x5f330z4wf',
    'vcm_z6bax4kvjdx6',
    'vcm_v8hf35cyq3ft',
    'vcm_1h9b4v6nxmf5',
    'vcm_781bzn81qf6k',
    'vcm_w0xjjsexktby',
    'vcm_awsq3q4c711w',
    'vcm_kttxt7wgb6s6',
    'vcm_ets26z5fpt8d',
    'vcm_wjrw59wmwcrr',
    'vcm_bebgka17n0sr',
    'vcm_6mj0x3qd7yfd',
    'vcm_b71ke4fm61nx',
    'vcm_srz3gas5ra4d',
    'vcm_k64jj2t2j05b',
    'vcm_140rd035642m',
    'vcm_5y93wgr90s91',
    'vcm_jwp6p08byjn9',
    'vcm_ets26z5fpt8d',
    'vcm_7yfwzsyvrpq4',
    'vcm_f1vq0hp0tes2',
    'vcm_9f05kg36q4fg',
    'vcm_1xs9c2sgykgy',
    'vcm_hnd9z2fxq661',
    'vcm_jxd74xhcszry',
    'vcm_tvk0g6dwg3yv',
    'vcm_vrsdv4ahterx',
    'vcm_e2debszbm8vt',
    'vcm_rh98fw9a9an1',
    'vcm_2bvd1gmysaxf',
    'vcm_2cqq1j33y2hr',
    'vcm_ysj7kwb96kcp',
    'vcm_ef0eaneyr72p',
    'vcm_t59d7vdxtqs4',
    'vcm_90hrepevshtg',
    'vcm_4st9ex13vnbh',
    'vcm_30fne697qhab',
    'vcm_qy7evy03p6tg',
    'vcm_775eze19tv0n',
    'vcm_4bzkfvaexbv3',
    'vcm_08v2tdga8v93',
    'vcm_whxeyxzw12q6',
    'vcm_0rj5b6npxfkh',
    'vcm_hv22emyfkex4',
    'vcm_a1fmtgthezs0',
    'vcm_z9fpz1p5bpr2',
    'vcm_2y8ck2dbppkq',
    'vcm_0c9h3mzc4kre',
    'vcm_m0z3tt4gytm9'
);

-- Remove duplicate models (2)
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'duplicate model'
where token IN (
    'vcm_v790w43dv3ce',
    'vcm_xmzm7eextc3n',
    'vcm_r3b91dr0eta5',
    'vcm_p1rpmfmckd5c',
    'vcm_cpajdgenjfw7',
    'vcm_qchjs52fsr9y',
    'vcm_pkkjy8c6cpt3',
    'vcm_f8ra6j8d6wgv',
    'vcm_mhsz9bjn19tw',
    'vcm_btd5zab2jdqv',
    'vcm_q5evptaaqne3',
    'vcm_wqtqs4gj2zmj',
    'vcm_14rv35fct2ay',
    'vcm_418j0h41qy8a',
    'vcm_3m5m6bpm0rt2',
    'vcm_gdw4krmaxpza',
    'vcm_b3xhdb8zymvf',
    'vcm_aak9m6nvj3am',
    'vcm_mcw5kn21bbjh',
    'vcm_fydr6ts21mtf',
    'vcm_zpw560jzyqpw',
    'vcm_w24wcctnxbxf',
    'vcm_ya5tpzdcvvcz',
    'vcm_1ctmgrjtn1j7',
    'vcm_rk8fycc0reb2',
    'vcm_e2c8942vky34',
    'vcm_wevg8cf3hcw8',
    'vcm_6hzbyg453xb9',
    'vcm_xqfffshe2g0q',
    'vcm_32j1zmcvn61n'
);

-- Remove duplicate models (3)
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'duplicate model'
where token IN (
    'vcm_46sbq0r9hfjk',
    'vcm_k8nseqy8jyte',
    'vcm_gs8qam6npe46',
    'vcm_djh15g49cxm8',
    'vcm_d7zygjnp35my',
    'vcm_334yqeq11rpm',
    'vcm_6m3b6121318k',
    'vcm_a419w7gw6jrt',
    'vcm_wtyszwjwq0ts',
    'vcm_0q9t2azhqvf1',
    'vcm_fx2k40ffzjqg',
    'vcm_wvjqmzd8y4mt',
    'vcm_b2635v3p50je',
    'vcm_zfyr07z2d0rb',
    'vcm_yn2sn5j2jwed',
    'vcm_z596ja3yvyy9',
    'vcm_a1p854bn1sjs',
    'vcm_fv24vmr93805',
    'vcm_0y7vsc2e9s3r',
    'vcm_5aqzq0d246rm',
    'vcm_trvge62x7je9',
    'vcm_sghga68kwggk',
    'vcm_raxcc33pj608',
    'vcm_a66b3va0sj64',
    'vcm_bd1rjawqypw6',
    'vcm_evzff14n721p',
    'vcm_jnrr4ht9f42f'
);

-- Remove duplicate models (4)
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'duplicate model'
where token IN (
    'vcm_ds2pz63sagan',
    'vcm_wkczgvq2fjqq',
    'vcm_z8137w1rddse',
    'vcm_gknzxybf4erw',
    'vcm_z0m1qtmawccn',
    'vcm_yek6pmgvbb1s',
    'vcm_qqhnjwsjrvnx',
    'vcm_8fetm7qz94cp',
    'vcm_kz1zm1d269j0',
    'vcm_rfnkc8g5gbbv',
    'vcm_4ncqxetfe7dm',
    'vcm_qkpfem2a153n',
    'vcm_z5jkd9r4cawf',
    'vcm_vgh6ja6zzj0p',
    'vcm_bd473tdrrzhk',
    'vcm_2k7tmx3ndd3y',
    'vcm_weyxz516dd2m',
    'vcm_1j4kpjvgsj6k',
    'vcm_211g17ea44fw',
    'vcm_7zkbj6be0yr5',
    'vcm_nnaaxeqy4xj6',
    'vcm_wp7sgtz6mhc4',
    'vcm_h5w7fzhxcpxm',
    'vcm_7d5rxpqkjy07',
    'vcm_9vhjvrptygfz',
    'vcm_4qbbawahnkvb',
    'vcm_am870m8162at',
    'vcm_cftdb5227qt5',
    'vcm_m7vv1mcxfxvm',
    'vcm_qn8vsvvh1rqc',
    'vcm_erhvqjvk1dsk',
    'vcm_4zs8tdrw971e',
    'vcm_4zs8tdrw971e',
    'vcm_z8htjaejr1wx',
    'vcm_99wdsazefjwr',
    'vcm_k1at1sbepvqv',
    'vcm_xcp34f5d7ygs',
    'vcm_8v8srq9sx7cq',
    'vcm_f2yf7bxmk8h5',
    'vcm_gvtrjte4as1s',
    'vcm_2s0y50avy43b',
    'vcm_r638ygfxkpmq',
    'vcm_1ttm6pxcpqna',
    'vcm_0ytfkxh6pm4f',
    'vcm_zderdrfmgq6c',
    'vcm_jvdpr6mtddb8',
    'vcm_9tvcv961g09x',
    'vcm_aynpmxbhpy7n',
    'vcm_2gzsjb5bdsdq',
    'vcm_a0vep64tz56n',
    'vcm_zt4hgc9pyv11',
    'vcm_szfxm0crd8y8',
    'vcm_3q89kba3cy5z',
    'vcm_ywpwnhk4pv4x',
    'vcm_4he8jjmengb3',
    'vcm_jpdfswn0empy',
    'vcm_2nztkt3rgm1q',
    'vcm_q5gyxf5a11zf',
    'vcm_sa7k9ycne147',
    'vcm_p6gxtj9qefsg',
    'vcm_j2fvzdyachtw',
    'vcm_ecbv0rhc6br3',
    'vcm_jrgwspz2wt7p',
    'vcm_f5b4j3rt180n',
    'vcm_pzajxdfcjfpa',
    'vcm_rq4f2cbbm6j1',
    'vcm_rdmfpbnab47w',
    'vcm_03dtk4xtggm6',
    'vcm_zx7mdkh9s9qb',
    'vcm_e0megdtf71x6'
);

-- Remove older models replaced by new versions
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'replaced with new version'
where token IN (
    'vcm_gdg3najz6j3g'
);

-- Remove takedown request models
update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'takedown request'
where token IN (
    'vcm_2af74np3jh6q',
    'vcm_8kk7355dc184',
    'vcm_nsh89pcx4569',
    'vcm_q7ewn3yym4gy',
    'vcm_cwngb21xsf66',
    'vcm_jmzjrky179cm',
    'vcm_hj6a1nt1yvh9',
    'vcm_6wwx1e3cd48s',
    'vcm_pc76gc1xh7jg',
    'vcm_gb0p1wmtk9wp',
    'vcm_pbs955dvaeq6',
    'vcm_x2wyzr6h6539',
    'vcm_py7qwz310qff',
    'vcm_k01bqvh0dseq',
    'vcm_5m4dsn2fwa0q',
    'vcm_hn9m98g1am3z',
    'vcm_f9168tyyn6ex',
    'vcm_03y9w4h99r1h',
    'vcm_yr9f1s0xc7pc',
    'vcm_45767r26a9bb',
    'vcm_rq3cvp3x3hnh',
    'vcm_ng1nz8d67z4v',
    'vcm_f5arabzrk4en',
    'vcm_d379600cr8yx',
    'vcm_p7m9jw1krrkr',
    'vcm_x7v3m9mcvten',
    'vcm_6r057m7594bv',
    'vcm_wkm9e42d3spm',
    'vcm_cvz78weydh57',
    'vcm_7g5q89m75n1p',
    'vcm_fg3175bxy7kj',
    'vcm_a7cejk4ng5nh',
    'vcm_dywsh786nmp7',
    'vcm_8q3xkanjt79v',
    'vcm_vkgardaf2she',
    'vcm_900cj3s8f073',
    'vcm_ah1ffqbrzzrj',
    'vcm_81x4c144phe8',
    'vcm_89ghs14h2gpr',
    'vcm_30jfz8hqdq05',
    'vcm_hrxhg4d13n6j',
    'vcm_58spen1nr492',
    'vcm_6qzhqded58bq',
    'vcm_wyfd0ft3b1dk',
    'vcm_rhhz8sgrxg4w',
    'vcm_9amtxj95fwpe',
    'vcm_5thjhwepk2rs',
    'vcm_p4xp9e34ypka',
    'vcm_1bknrg4gqm18',
    'vcm_4rxa4qp1ek7y',
    'vcm_71pgq78k1y3b',
    'vcm_7h3p6vn9ts4n',
    'vcm_7w2akqddh758',
    'vcm_9ytxkwqk00se',
    'vcm_avyzqxctreb8',
    'vcm_bxz11j5g503k',
    'vcm_byxtz4gs6m25',
    'vcm_ch8xjth7wm5a',
    'vcm_dg57xngqxdp0',
    'vcm_eb6htakr2y44',
    'vcm_k5hkg05p5f19',
    'vcm_kmhs1a6n97ez',
    'vcm_krvgbm1wc2xv',
    'vcm_m3sjae12xw66',
    'vcm_vn1qqvy0t9bn',
    'vcm_w6hz0tbmmmd3',
    'vcm_zmxv2k1q9jd8'
);

update voice_conversion_models
set
    mod_deleted_at = NOW(),
    maybe_mod_comments = 'takedown request'
where token IN (
    'vcm_3mzcjkf78x7m',
    'vcm_7k2t1wnyqxsc',
    'vcm_w2edpe6q2nya',
    'vcm_vz9cg294xa29',
    'vcm_0z1ys58589k7',
    'vcm_4x8a30h1r26z',
    'vcm_0vf9prxsww60',
    'vcm_yxbadkvqhg16',
    'vcm_p0ezrc6wvzak',
    'vcm_6r5pbtexyvtp',
    'vcm_pb0ewrf7ych5',
    'vcm_7frb2xk0cxk3',
    'vcm_x129c9s2af06',
    'vcm_e0snndfd59b6',
    'vcm_nb8zb0djqwqw',
    'vcm_2erz5hbh8tpf',
    'vcm_e9wn2ccqe3bk',
    'vcm_f8jbxdbjgcjb',
    'vcm_x02h7z3jqxb0',
    'vcm_xy97xcgnk9y6',
    'vcm_a47wtfbsjrfc',
    'vcm_kygenhbg5mgj',
    'vcm_s8gndv19yt7p',
    'vcm_ny9pj57w5eyh',
    'vcm_jg4rst3j2d7x'
);


update voice_conversion_models
set
  mod_deleted_at = NOW(),
  maybe_mod_comments = 'abuse'
where token IN (
  'vcm_qajyd528p8kw',
  'vcm_f5pcdecmzcc5'
);





