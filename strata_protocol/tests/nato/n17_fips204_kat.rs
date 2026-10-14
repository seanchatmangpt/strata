//! N17 — FIPS 204 known-answer court over `strata_protocol::signer`
//! (ML-DSA-65, NIST FIPS 204).
//!
//! HONEST SCOPING LABEL — PIPELINE-KAT, NOT OFFICIAL-ACVP:
//! The official NIST ACVP ML-DSA-65 known-answer corpus (ACVP-JSON
//! prompt/expectedResult files from usnistgov/ACVP-Server) is NOT available
//! offline in this lane's environment, and no network fetch is performed at
//! test time. The vectors below are therefore PIPELINE-KAT: self-described
//! frozen records of the signer's own documented seed -> key -> sign ->
//! verify pipeline, pinned at lane time (2026-10-12) and re-asserted
//! bit-for-bit on every run.
//!
//! What this court proves and what it does not:
//! - PROVES: byte-exact reproducibility of the seed->pk->signature pipeline
//!   against values frozen at lane-authoring time; sign determinism (3
//!   runs); verify-accept on legitimate signatures; verify-REJECT on each
//!   of 64 sampled single-bit-corrupted signatures; payload binding (a
//!   signature over payload A refuses payload B and any other divergence).
//! - DOES NOT PROVE: conformance to the official NIST reference vectors.
//!   Upgrading to official-ACVP is a drop-in extension: replace the frozen
//!   hex below with ACVP `pk|message|signature` triples (ML-DSA-65,
//!   deterministic variant) — nothing else in this court changes.
//!
//! Frozen-vector pinning is the KAT discipline used where the official
//! corpus is unavailable: fixed input, fixed expected output, bit-for-bit.

use strata_protocol::signer::{
    AttestedPayload, Signer, PAYLOAD_LEN, PUBLIC_KEY_LEN, SEED_LEN, SIGNATURE_LEN,
};

const KAT0_SEED_HEX: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
const KAT0_PK_HEX: &str = concat!(
    "b0ead780af27b7f974532cd85bf3f86010b571826c9faba88183ae27b9746309",
    "269d3593c7001a3755b41d3eb42b7aeb172cb37bfd219c87f23fd7ebadeeda7a",
    "60239ac9064c28cb80f3cd95081a9a029e8e88b36231930178225414892ff965",
    "405f75498fb2cc1b1eeb21d52d50602da1ed2eec3c060ff09f32b80d06742ee4",
    "f4980b419514c66805f372e021e71fdfffe754a6d59d11d32e4958a9b09d940b",
    "3f72299bc69489e03dd73cb8f60f32e4f712ae460fea82fe928a0aabd251817b",
    "5b7436256ab486cc6ad289e0edb917bf1e051f30a3915c8787140fa56f26f225",
    "be763c57cc3f41a4a9fbb69957730d08e55918e6516b0bc0407bd8e2f6e8c7a5",
    "aa9c41b17ea0118cefff7f73ae6101259a3bcbf03fdef3d05cbbddc2cc71583f",
    "7542cb569a61e311428a73b3161faafbcf195f44e5c67de481752b041848c99b",
    "70fc412617405254d1a4cf173f3df738b76582835059373e3ec5eb736b107345",
    "ba8fdb0fc6808726baf98f9bcef52cf4673feff24f153bc051466f8f42420951",
    "3d141359a9ae65317464d2a41bf73d9da81b90b60b1181aba2a28b0b665960aa",
    "251d24c2235897aff41020b0f1e7e8290d8ce6077aade21e5e37650ef6cf270a",
    "044ff84f38292cfb2466b184ac8f6b873384ab303addba80c3f42e8d84edb016",
    "f9486648adbdb00fb27916186975ccffa40a71123d7984e97a70c8d343636886",
    "9c809bc96f59c6c036809b4051d163e97826e94fe349d5c2f6cf554459dad0ce",
    "404d4052887896897b19562190661e516f4f283c39ba9030f578585d3ffdcc9a",
    "28e57625b682c7835151940bf7136614779db34214eb1793b8544c8112b86e7c",
    "c7b883909d40d6262936c62d002bb642d116392795e50b8a1603e0110d8d046e",
    "8a05af1eb90cb6d308a2d3fd6b9e227f0ae2f4bdee31d27d2f0c6069acfa21aa",
    "4fa56c1dd604a1069103f6cc806df72b0c3132fcd4a213b372e43c436b8fb143",
    "2b0d64827acabfeeee05cce12a26e154d6cbc3d7ef96d6e56625a10afb650a31",
    "9394b9f63a0b465bce594bd3aac97873873ba1a78f277c000a6a0811929dd397",
    "e079a00ad0ad4514d1d9cab87a90906dcb11f5be434360959bb3cdc669e1144e",
    "21652e465c77a3045a7421b26873fbd48991db4d4b66fb05bc2f30fdd7308740",
    "c26a1f252563ab7db34e1022a869c15dc0f1b51709cfd34e4c1d8932658ca0ac",
    "e0ec2622477661a015eb9444b6f466b1f362c9112710e66d0681cbafcff7f177",
    "8e0e416906a75d51883f62d4c0db1990e487b3ca6be526a59ad4a996a0ae275c",
    "f9543d2999daa6cebc3153f5967642ef8cd502a224efea507ac6f589b8b04b42",
    "263c54b3c721ebf1b112c8b91f92865c8f1ef696364b43538d63893f54029183",
    "2816dd6827d737d82ddd85b6637f5111625594c531fdd316cc41f9541b34c511",
    "64d210d3d98a2e9930244cebab12ac352e4dec35391fc7e9df53dca4d5a173b5",
    "b5bebc7f2c91ceab605d60a8981b95800351279d419e64b8db1b2a730ac61e9b",
    "d60da1152115ab2ebed91a19d5f965cd21677d8a9fb8d96607575a72fb54c7c1",
    "da9b49c6ac82e2f9660a34c15454779f3d713046931e9dc0d42caa03ef2b4461",
    "5ac54860ec15e068f30f70a150f09e902661c53dc2b89bcf4acbf946529f4427",
    "b43025661cea10ba0f1d9c0b55fe6cbd9a28118c2c435622ff55070e194e1c7d",
    "7e2288908540252703006d213cd5183b2268a3ad544cd8a4eb8e1a5ddeaee4d8",
    "f2826ee5d26d13c86256231c9a3965c88ea2494ef2518efb873907bc120ea5a5",
    "a6c3237e8efc6d1937833b3ed90effd5147fc695cf47fb9449344bf725c1eb24",
    "89d1c3b080a62c12301de08fde1746f47efb8caae3e4aadcab98c82b599d23f5",
    "4d49143a3bbb3736f1bf803a3ccf046ee95a48c6d1f08da8b415bd1f24842a10",
    "affe8c50203f2d4fa8e228fb289aa01bcb1dc7caf862284e622002ca0d3066bd",
    "2acf9fafc125ed22b90d39f7f34ca777392cad1358790b8a6177381ba3842d2c",
    "a010144e58a31a40878e6836c0c446b71e1e45359889525dce511dd41762c38d",
    "e9d0cb3a05debf6dc9cc3561f82a7e12fdff1d4bd008280975d64a2f7759b959",
    "f11090f02d1f26b2fd8f9d1cd19b00a807787cff3d10539c6b37f1e7b2f97a61",
    "e49e234773b3ce0c949b704e15bb3dd11a43a5066731a9837319b31b9d3a4f03",
    "f8108c3c93b3e0c275feafc51172a611aecf905cb9f8e10d964d05ce8d9b1010",
    "11b261c49c40b5623f22f8a7945bb808eb3cfceba702f934aa9e6d48291a8b86",
    "ad0f9d9736002d04d4b6c96f3b1db9a39d0d82d9d8f95699d437443f3225ec65",
    "913f2fc2c9585b2f69dac102f41959eeec7fff5b6f0cd5238f845801370a3e17",
    "7b78cf6b3742e02941000bfc199dc62a79ed9a41e19cd275dc2aa9079a51272e",
    "c5a10fb4f33bf1a0c335b9dff7b6319b80cca53bb9c9d80dca50abde5a77c001",
    "0515b5ee1042fcdf7c995032e945e0cde5741b73f359ca606f39543704ec6d5f",
    "c2fa1815329d75abd45452af51b2eb51f5103b6daf6fea0efb0cec3ce2f8675f",
    "92acfd76a3d110cdd6809513b37694f6a697de2efef04d01128a8dd48bcd90c1",
    "e21d0a286ed612c21ce66f6f2884b71bd3f8963628054b1ed826cbcc195be72d",
    "1b6aef1a531ea4557dd92ee2774c3c8a4902b53224c22013afba9e1f1fac83b5",
    "7a1c351935ef569c8361a4df34d64239c86552f575f1ad74d90cde8be2a93ec1",
);
const KAT0_SIG_HEX: &str = concat!(
    "2d98788de4bc5dc70042d934438692f1d689d16e5d8d2578b6ef831fb73eb8e7",
    "4f5d5373ff25a1c3bea3213d105ac8132610cb40679ca0ee043c84600dbf866d",
    "a1fab44b550d928f623a03c2be572427cc4389142da084b8ba453126b3dba990",
    "72bcf6c13ce1b6722ee6fefe9dea525829e305bd76365ce32420f4b88dfbc936",
    "699661fbe74675a497311f15a66a9f1c35aa6232daad4057095f009c54ba89ba",
    "1fc256ae3375d5562d67bc0e33d26147757c3235ac4798f00481634f67b07841",
    "b88fc012cb129658d8d2408d99bf3884618fbf9c9530257a9bd5ce6ed469be0e",
    "22065d65280b3d8f32f3400cd22ab15ec34dc8ad3ab3d37943ebf051c8f3d8d9",
    "413d338125c604cd1412175db269a832bb2bc39d54807cec28a1450b0c987a64",
    "2f3f4e8bff67ba681ba8d4544202d254b372d6e4bfe4dd8bc27dfea4704bcd85",
    "952643ea8c044ecddbee8b87406608cbead6cc2c41ded49342e9523824077c8a",
    "555d65a1bc6e3ade974374333fc8875aa90ffc4f023308c42cbf0feb335627df",
    "d1a057c793234b6479a9767ca475f7d6c9f4fa6f9b6ce1e607ae29ff736ebc2d",
    "89b4f0c206f12269604830874e18e31b68070e1bf103dcaa3a36b5d2df38140e",
    "356eb913ce87a0e74e83d9bc1620c1b72ce7aaceed3d5d050eecf316768efd52",
    "1a20a270d1f442995edf22f5dc512a96bcf22e2073f07e2fc2e34e801890f840",
    "4445211070f844756d0b90751a20baba8227463210589a06de9d60a0c65b7289",
    "5bc03bb440c1ac82948a4733b17b455ef0b009d24bfa3d7ddaadc410b35af9f8",
    "43289225dea78945d51e439995aec9aff6efb69bda4f71b0d73cd8b01eb28cc4",
    "06a67766d1fb381cd4cf21435dbca2c86aa827388daa4a30f8d1a4966f5e0746",
    "59302fbe8e1533d8b8c206d6f5c7d2d9784324d47310876c99ad08e4d43eddee",
    "52bd685939749045d9f58528adcc5e108e24b4fb02bc8a5bcfcaadceb5bf4a7a",
    "46353458590e462b585742526dc218afed70b0bc2ced6b1077d3204bc7deecca",
    "a4032c0df31804a2aebc53a567ea4150cb01b3d3398ab1539623ffa988ca9dab",
    "aac930539a3547a29d2a11286e62bc94ca0ddc69697d0497c85512d766269240",
    "d26fde6ed6b7a6a5f78fa479ecfd5ba1946f41bc42d908e65b6a0cce09ced75c",
    "614ee1832de7414d21ee180a4b5588e61ce1ff2f63a8880b9f31438bf95ad70e",
    "126872138f2e8e522eabdd8c271d633a138b6aa0df8a3bfc88fb4a51b396f10e",
    "909043325009b18a24e4e8c5297fe534886f155d50030a657571a37959b28b25",
    "a67082aff5803cf8c304dbef41d5a753b8637f9685ee4921f50dabcc44e07aad",
    "e16f486f25797d05294567282ddaf200f1cd59d759d0d6ac6685b66f9ad0ac98",
    "d9ad08b82daa4ee1997bb9a8b889eecba7968dc3f70fd8a62fb4587280ff5de9",
    "a91b694fb3f6f7365e8c29f76129876c8cd4a726b498a21d1940e56875baa838",
    "683c713efccef3cfa150bc562364837da7cc11a5f5bf1e51ef27d63531c78c28",
    "3ca115f3ef1e8296c41d7d265c6860bbbaaeacd6b0e95c4b28d41c4165f107d3",
    "f0b6338b50c841449c389e465a81099c7ca88d5d63e0504f972486fa3a432680",
    "b31de7ba2058386e6635a0a259ab02a5ba3ff9b2bd3db324dc684d2cd0b4693e",
    "38a484431212007c5750a1c530674584acbe31391ca70e09dfbd0243b6ccda33",
    "0538f31ab72baf664131e608f5bfc9f2656757e1b5233e681345bf0007e40a8b",
    "9d751dd262921f0bd3a7954f932207d8755c714179c70fe8ae464e479911a8df",
    "f904e9c436fdc684a61a8cd23547652fba961af385a47306a11e9299fb6f3a47",
    "afd0b40fcf81d8938f437e51a9f237baff53995490c83ae99d9eb5f9eed2d09e",
    "ce77ed2fa97ed21dd6d84686ef4419da4b58bc68efda275bce13aa471382af2e",
    "7c0aa4823af4a56acd88513d2f0913c7983b1e53af9f0b8760ea3d788af6f4cd",
    "bf304f53fecaee3d7f8e2591022cd99c036b2120096fd01e896a24161c9ac052",
    "76c0d700a687a9e98b89e1ec07e1b8df740de071eb0a1fa56bb6fc3741676e20",
    "f1085b0209b5a093b6a43020288fcd71ec24f19ed430476ad9f8a1eabc7cf420",
    "586b7db9654663061a4cbf818dee82f4f2eb9623432baa02aced8b21edf609d0",
    "e439a85489246950af59b263247382e292b229c57e5d8290579f1e0f0e1e5e86",
    "5ebc88c4fe44d43c2f42f5351082a04cd5461da8f563bff2f3111a789ebed3f7",
    "7e54bfd0dded0be9b3606bf554410d8682198cf221a0e20df566eaa5a173d223",
    "9b1b6a6f6c3fa4881241fd0c2b83de716f677056f20c91aec2026d5e74c00a63",
    "a70f4c904c7b9058bc760eca3b90b23901d3bcdba1630477cede0d84469ad1d4",
    "60ef313c45ebfd0cef43a38a0dba499c9395780780d055135ff8f0bf02891323",
    "d49b50d58c4b08c4f887f0d046b7bec45815c356f2a211837eba9594087e8613",
    "92f62c1da6d268d9753ef44827df51ac0b1675e179c9cae161aa19f404f236da",
    "040f8ec5c5a80a4691f22583797e1d5e98f2193234e6a777d7253637e6453dd2",
    "45ddd4fe217ab3626a31fa735210f7d051a939610afc085817d809133160cfe0",
    "1d02555c2ad6558c213e567a2853c4e26a090bc75eb49f74e4554d9d4aab7705",
    "ff573c822e254d6eb1e835dfd2ea598e8f97b20ce0335c93d8a8dc3510abc203",
    "2b075cc406d635f35cbe420159bdac2d4e384da412f5376716fde13168dd45b5",
    "f9d2104eb3d4e432ab338cfc6a4243c49a6e8886c9a4b62d456c88ea8c874da0",
    "3f094850eb3f36a832bf717908aa80b3874c4e142348648793e8f9e3d3e9a910",
    "d02101c8975df72696a883b564ef60e7119c61057be5a69393e3dae6d4e07f92",
    "d71938b40f01098360a2221496d1044a5f867cfeb85b57cdcb7d187ce627cbae",
    "88145069b395358fd9dc4ea1624d8f7db36ca8569c7acd3805c63d3cd4fde436",
    "a7dd6562bbda12a59925161dc8e4da24fb629aad1a9b6aa7bb6c1d91b10c2b94",
    "a486d509b747418239385c4187656d872332a404ce2e2bb19ace4c110ec3c777",
    "924b9728860debecf1b2d5f22920dd1a58efad47853982a8a95dfc5f3e58a6df",
    "04573c3d372e239047cf1e75f2051a4f0d5241f7661d087852ae4e531b8517a0",
    "6f4388701524f1dcaff53ba59093a15f3b6a6794d9c15a4dc2bb7ff1774732ef",
    "c2addb1308053bd0ecd72fdf4776c9f207bda8d89d60c414f2448d19214f5aa2",
    "36d2688e5cd2d30490a0459d2c96a025c4ab45c905e98d8f2400f51ec0d25038",
    "2ca8ae09bee355d36b540ebf82f9f924062305eee7818d9fc490a29a3dd4eaba",
    "e7e1f233be19eca95939e03eac9366ac35ce3bed93a13d416bc1bc9633273531",
    "91b67d92aa2a990949ab7a68f363209a16d5ccecb25797c36d90cf45f0182355",
    "fd09a211cc87213df5d3ff78a332af53cafc28a264c036ac4a84f2d22c1c688e",
    "701747856b064391bb748e04a321a52b29a1866f89382d039b63b6c4f045b01b",
    "48e1d2416c5bf0be4a595186c8d22933116d36c5cfbbc340b2d91c4dca79e969",
    "01dd3cd842666db1f6e88e375adaedce272ad3d2a8da4d2b6db3468d30385133",
    "9c141e7905ae4a1389a58d9aec85ca3a2649d01b474280577d782f748c421145",
    "bcbe24868d6b58ee517e152ad84889e3c43ae5989cc3c7e7ab3f28193f3fee2a",
    "9bc3caf2d138658853ffb80d392ef534d4b3b06a2d3130e6be3a66b71e7940f5",
    "d4c51fef71e53b9dccab3ad5f7e2b0fe64397556d66db76c1061970f76852b58",
    "0fb6c444fb4e060971fad047e2eb43922068c1174e8604467476817b8ea743ca",
    "b73920571e4bf5bae6dee39f8d37d3f612f587357fa54c8b37a1036a5d5a8f12",
    "7f4e2fc2c2b30ee781a6329d4d9884b1bf56796b6ef215182f4d1e95a6246ab2",
    "2bc5eec4a373694920bc3b191d1aa1f7b5d9ee29fd8d0b721fd773474c24b384",
    "b35c7f5b7e6b5cb3fbe052852c40320f828fbb031e219ba1390392ba0438712f",
    "d3ec2e17d7ac62578316dac9835f877ccf727afe1a258fe46c421b7d3ceebb76",
    "eb3b767d7ba0464b6a8319c3075a62519aeea3a738dd5304fa0b15b46eee6d10",
    "9cbea52c5dd620c05467edaeff4034c4da641cda16ebbb91b0b3b55585f62db4",
    "2fe0225aa54482cb58db2281feed8ca130957e995ce1a1b4b3a857804eb7df3d",
    "4fb3cfaaf25a6c9782b24a51101ffd6e05513ff705ec01f994b131dcd7b2825f",
    "1ae7aac964cd4fd55004b3b0f9d959ebe90b5318d4a16d37a620bd9ca6f15fed",
    "62ed3e6465c1a945b39b24c7e3298a0530a470083c813e4fb0ef8530cfe92526",
    "b95c2fc1d92d5af017eb440cd22c6b43427dc6aad0576721a63d92230d5dc437",
    "61ba1c61b491b5dfcda64f4ce95d3fc2e070443b36a542ec7a044557c3fdfe2b",
    "7168a6d90ddc644f11e907ae88423f0cb1561ba18f605166a91825741809ca1b",
    "080788ff4d5b56428c57109f33dc709bec896992d44152688576d5b1346c1c68",
    "7e49807b569821fba552a44298ba301188a9bf909b8ab56f9d60e218c175814c",
    "47550e178161b0295ee590034c5e80ae1432a5b4d4ddf81b4d667c94dbf90919",
    "4bbaf0364d8caaf31b2127496f788790b6c5d5e53556648c0000000000000000",
    "00000000000000070e13182428",
);
const KAT1_SEED_HEX: &str = "02030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2021";
const KAT1_PK_HEX: &str = concat!(
    "9afef91a51afcc528450b74940e5f8d88fcd7071ecf0ab4233a239c53754b088",
    "14b651dba50e7ff1c834f6eb4865eebb259f66fe94247fd0bdcc6d58532593ab",
    "94797b9131d586d3e680ba32c64354cae11827992d5701a2e5ae96e29d25d097",
    "1226861cccd444af27b8a4bd4e61238fdd8da6346105aac87520145868c6dd85",
    "a2757bee021929e11b68da3a38c8f379fd9ce41cba95ee59531bb2723bfed11a",
    "d122bf6978cd26978e9b039ccfd43cadd092385ec1da0a89a7382ce4772b5986",
    "ccbff2984cbd8e920fd77aa3af3cc2e20ba8076658ef6c2db9704f95ad8f5427",
    "f27f77f7daf254a83cfc4d0da6463eace4957a56ebc309c175c96eb4b18d28de",
    "9ea04a5cfa8732a6db0559e81161da2bd71e9516ef41edf257bdb9326d579fe8",
    "87f635515e1582dafcb277bd42f27469476be877dec1bfadbbbdb6fc2fa2558c",
    "bbb9c9091b76abdea6adb14157b9fb526ea7f100c9b4081ce20394f8cf93861b",
    "d6356581c16614c912fa87ef1f9bf6babce9dd120a49fce5e6bceeca965b3b98",
    "45070fb8640f48e26f4b77eaf9125dc2d508b5e22301bc40ea66d459057d8a1c",
    "7b678819a4e21ab0bcfe440f3f542bca1b9a41e312597b23bf43f3778ec67798",
    "fe873d9a9cb3d8ebe4040e112ebe1147bda8b251461b0c4f24c633c874f829b5",
    "be3f4369842838e246d31004449d04c10c9d8f53dc3cadcec722bbfd405464a2",
    "e23fe4f014c96ba7bf82bb91c21b51b7879a6dcb2a5e0b0805bfbaa30fc68d44",
    "15dfc418b5bc6434dbf4ae684985c5e2799796ff513606327f820cf9147bb3b7",
    "ee70b66c715fa9a34a1586d9e80323a207219a0c4c9944b85b586b6fe466338a",
    "2698220104639e90465003a705cf2c71497460c290a03d3be8563a18b31c2ada",
    "f4f4994fd51e532e443f43197eebaab64db60f8e51bb54ba8f9e1b025dc46e52",
    "7e48eb61641fc472a7401f04d031fe9044c7b07b25930ef6e80022d713041789",
    "c717c99777b32bee31399ceb30416be334cb11ac1e3fa1b5fb120aa9bc466ff1",
    "184e8bacc9beb6191052dbbe41b6d82eedd855698827ed5dd715af97b654bfbf",
    "edd318c625d1723079a7e7ccbe8d0f444818e61843fb1641e09e663ac75503b3",
    "53cbff82a213ae0f832761481b5745506a7fb6661a6205e76d9160f18758491b",
    "6a440b98483e5b7014036d3d383058e69bb1758e284ac19fb4b596cdea8bde6a",
    "6125f9f2e625fadeb2c5208197ad34f4cf2af0bf2625152f8cd37aa0b3c306db",
    "8f3d2f5fb6b1023e453ced94a39f9e837c67032df8bd490d95fbd6b05841668f",
    "54437a09076f3d88576ad32072096299dfe5ce8ba9068d27f70862dc0f75a54e",
    "71daebb7aa7530713acb47a622184cfc69d9664dfcd23e22474dc2923f3af737",
    "374d099f3f5ab1008659ce1c6f900966e7ba3c9fa6575e49f18cc628a939fd28",
    "dffaf9e69d9e7896a7ef7eb79b83ad991e6e5b540cc3cf6322b734010eb3713b",
    "c12976f555225529e15d2f74eef7e92ae254ef7249f84c8b395f810a3e6809e0",
    "b400c23102139abdb2c418f278f8ca3823b47fb6dc535cec3d9d34cd93efd433",
    "6a15b7ad218be6db17ce86b2817276ab3e591da2bfc7b82a604c8038911fa8ce",
    "9a9d24911b26ed0abbc38c67c7e02ac59f0f71fd3ec3223f765d6a600558ef17",
    "51948d018c871a67582e0675a0ce06b1c57b1546ef967df3b3fe47bc8f854690",
    "223cd19d475f11cffe083c8c2920e5710391eec7319f583061a7330b547a7120",
    "babdf445ec77a6be794df02dc10cef8ea2dbb64e0a0d71333fecb155f7e5a6d0",
    "00f5e8994b6276dcffec9943c996f5a537664ba971a66ab8659cd5893ed94289",
    "7ef15de582706da891cd5c117d56203dec47d0eb1fdbc6245c43076a5deccebb",
    "c4305876cfb80321cfa75ba981d76bb5a158ef184e784425dd9e2bcceafcd5c7",
    "b4743df5c3e53de37f41127db12f427236e007d04aa56ff7a06bc20a5f32fd1b",
    "55d7dd9cf04024e77eacaa9b8101af16298aacb6782116bd9198613cff0e119e",
    "45ce5e503c9ee247f537a07ab6a179a4bae89e95fbb29c0b58b636c4b6a91610",
    "b3343a3ad5fc9eb8ae5e41a9541f21ae48babd6524a5f8378345785146febd7c",
    "629e536cd5da8b61013db052555619e673357c78c91cab70b319c5ab1891444e",
    "228fa0af3dfe84384f5e2ce9f38924939d4703c7adb8243372954e9fc217b8d2",
    "cc2a6b8c53ebc74a0fb3350129a4f5378f3b7b75349e0ce0ef33bff02d53bf01",
    "b7b16501c76ec24820634cba4c921a8ddd8f1b1ef6cc33a87ae61b87edf2dfe5",
    "d368d129b4db9a3b15a4fa18d7c04f9a6859d092a15ff38c432007727f16b393",
    "6d176a28ed3996608b5115fe30db1bfd957e420c354450eb92f686cdb2350fcc",
    "a525211e363fad967018f2f7163623b9a12121fdc1d55854ae334c22ff236a4e",
    "748853ab8dd02fd4564af4eb870cf96e3a46cbe47f8c9133fca2407d565bf238",
    "5a4d399cfee00fec2a94b9b285260f9ac69505b5c63706f819ca83c0f0efefc3",
    "f491eb59aa963e1068bfe51a547c09e8d9065e48b3ebfe5c300df8c06c802fd4",
    "0a9e2e19984a03931619b825b14345a4f5ba68e4e51b6e85246a9c7486bf63e2",
    "e418e23ceb9f7ea422a44e0cc30646a2cb895bf6cc45a46b51f00d337e0e46ce",
    "5ee9726ca12ff61202c5349717d2525f68657badb0cc6feac21bff12e330d6f5",
    "f88cfe5b613a4d9b9b7c4b124ac887f5c0913aecd76add9bc65b57d067a2bfa4",
);
const KAT1_SIG_HEX: &str = concat!(
    "2641c4d16478d4bd1cc02402820eb0698dc5ac4f4eef77df9836ea385c22eb27",
    "107448647b7fd57db6505a24d0d9b39eb48ef2252140cb3047b4862756c97336",
    "20fad2f5f21c58dad1a0aa2cb529a8d25a697e65e44350793a05a5b8ab052ace",
    "c8606fc2f39f7d14936996c4023a0065532f2f0daf438af0f476c0178708efa0",
    "80cc672f69ff54886f1618af904037470a6b424c90e6c5d9eac35ed1a33430ad",
    "0294e9c68fd7a80fd38189e344cbcadfe7bf1c16b737549acadb5eb11ffa9290",
    "743f762ea483f8934e030e1bec7e931e47419955f39719defcf8e8b880b2047b",
    "b554499f796f94d8c67b194fceebb883fe5a6d84017586405cbb58905f0bb291",
    "471d95881cd477af1e0dba58e8651807a7888fd55930579c4440ad167cbe5ddd",
    "64a101b85bae27d22b4e0ecbd53d3ee29ad9587b9f8a884bb0e31a60028aacc5",
    "5755d075d7b4b3540d334656ebb9c436a445cb6d7ea9b25e6aa7da42901fd156",
    "47d4c81fc4a8b18ede2498402560df628cf2f612880a4f08dee4923a09a75cd0",
    "76a44f06e7894b09bbf236e3eaf0e2ebfffa80bce1c9032a8de54ab96f907ee4",
    "478b5c486cf129feeaf2fe28695685c2cda4ee395d6094fdf9cc17728306fd71",
    "8d568df630e15378a46ab00ab5150f9936be74839ca76f21ba8a7682495de368",
    "9f87aa558f5165c4ab4e8828b9d0c7d4b0f7124fcfb7866e0270e6e19c82b8bb",
    "bd736d54ca5a431c7ca20858b1a8e4e2c67406755e55ed6b83fec166415cbbf2",
    "704634c2f9cb69e322ca5bf5e8564f528adcf395d142d063b3e3ac2ef0c1fc0b",
    "618bed66e4558fa9b5338b8dfab9a4c15da36d37adf4ccb1e8dc647fc7e5c74b",
    "e2af9575a00783a10a7fcfd0ae82309423f514ca7975f09c92dd631b434bd273",
    "8dc97305a24b60a812c6dcaac73f858667aa8c197e39a9793cb1745736937399",
    "2d0a44a1b7465dbe47082627bf9c099cc69cdd4b1beea68b2d7edad46b5d6604",
    "1462638ec04290f9ceaaa99b92a8d16a811ec100dfe910c356fb6d4fd26b3779",
    "39a17b391dd8b7eec7945404193f6aabb109c05e89f3c2b05fbcfb2825633775",
    "e4240d66f8a87ec0ca5cc19c75c8396d9a58886da9421b280ffd52223c03707b",
    "c58550c29df939358c73259a635a3fea76b780749506b5b11de4eaacc7980586",
    "0d8d6d942a94dbd9dd3778d2b60364ea72816cabebc99b0e429a9a6c3736ec53",
    "bb3ef13fcbd8822ab39abc708987b3ef734ec1cc77889659752771aca08b46e1",
    "cb4b82a24c4a39c1e4ccbad6dee21c2a5d1305134c1b4c6e34f809acd21c280b",
    "e554925747d908b91a6875f99cb8d2a74cd55643f655698bc697ab9f284f2d2a",
    "70114a448a84d60958abadcb56b5fe031209dfc20a93e9fe42436d89ac0a04dd",
    "fae0f3cdd7e13ae5eb45b5eb6e32930c708d8d21192d812580baa34ad386a279",
    "221ea3be97e3d60dc15245c845f0ea67440d72e1e22af5acab9f23541fcd7620",
    "c59b4a12e7617bad0f6ad9c4918897d796c61e0da1f8dd671db94e90273d480e",
    "4f808c1d731c4592f76ea12203ff2ea52ea5baf47c65b697b8e764ae9a15b6d3",
    "6b68ecb708031aa12f079657425273ae3c819b3a4885ad156b29160ba6338a75",
    "e748df75419933931cb618491b29026efb87cd3e270b2d5cd3f5a85348a2726b",
    "fa714313d0952e2fd585e38afd50f255ef3687d11b78858defa08d64c97612c9",
    "7c86bfc27868701dc2b2081fb3bdc65d1739d6fcb8d92af0d68829e07ea3e6ed",
    "ba220595cf0814e8104a12c3c1671004a77e4a68c42ce77bb8a802d7d7814e02",
    "ea5c2217c0920068b329b0d56e4e0f1f40cf30f9830cc3ba2d83b070381e4ef6",
    "f6b1ab1f66d0046fa2845fbb49e220428c78586af923541d7e14853c1ca89420",
    "693d3416d6ac1702aa8181ffc396d3ea134a3338c63f4b6a66812f593e512dea",
    "52730aa08f2eb52e46b4b4ebd704576b348d22e23252ee3ddd09b45034037dbe",
    "9718be0564575a0661da93fbf3fb3f721b4cf9f24ae6b7aa878610f0fb0214d3",
    "cae8a8ac0ecca79f4de74e2028a3be7072f2d22094923c85f7f112f664777412",
    "2066457203e7ad19b7deb68b666295ebf0e5b147e07bb4a950287ae024c62881",
    "8fdcb0e7323676ab9f184e66ef62f1663309d8ad0b743681425bc6692bdbd646",
    "251e31bffeb692efdaa57029c6ed32ddf8069efb46a8c5c5b60dc39ca1c975d9",
    "960bb688924838165af8db2224e41d98e0209abaf307303500a3acad510428b3",
    "aa3ff7f30131e31a7c6e433b0821ed748de4a1caa79597f6c50ed8b1d9e5f96d",
    "f0470dadf7cd028dc806db1be82e3a9d5d7e4e1b465e59334b72c1bae85892aa",
    "a6fdd65ee851dce05883a967f3ee7b559b312d77ada2c556d7c293e3d98bf08b",
    "750ddbf9ed49599ea7882d50fe32233c8e30da6a8356b0b0410ce869cc196ffc",
    "018cbe64f8b189c794467bdfdda25733e5f1a6e0b1f81dba62388c3f4d2e73fa",
    "306640974700f5ef46ab8e143c1a0121473f04f60fff95630422d09bfc219a66",
    "1658d97b74fc2f3687baf701e2a4690159f05d9a0dcbde1a1775e063357060b2",
    "343c99d9046175eba54752c75df2e091e47ae77e39668517fae0ff4e1d98cd99",
    "72fbed6656947daa3b4ab148b83f52dc237d2206bf73a957df2275320ea83f28",
    "a1172e95fc5f0e66c322fad0d2b9269ad49847b8169e39c926e99b0d4d63e317",
    "cb31acd222920712305c97d58377ccb7ae29eab3fe206c59f31e97dc3f94156b",
    "bd1f615d22c99cb365dd8c2c4d727a1f8b057f01317c94ce0cbb28570c0a76b4",
    "50568ae22e6609903e74b8114f9c62120a5129aa61523bf898003903d21b8483",
    "cfad151b96964b3cb3d8fee03524f94deffed9981e4725aeb859098856bde59b",
    "dd15454fd6c798aa4862d9714da200b5aadd5943c1537c049837f2dd234eac9f",
    "94c68e19c2cec064bebe42c780e7dc7fe967c3ab8ec24e83defd0cdbc44d1e56",
    "fd195b9547dbff4c7086945a3ee04d96a5a10b8e5aec20871d46802685925f4e",
    "92b89bda11df21fac68d7b67a96bd797a069b2c898676d16af3dd822356fbfcc",
    "8988d79bd1630ffc30eb3ab08bc00a0c381b45e56c17391053d5d455e44bec6c",
    "053e74009ca302a1bcb8f1dfac79b48f463abe538168bae1459f29d594cb9f62",
    "244e14d21a1acf7ffc04d08db6ed1a1873f09b25463c9770561bce2bbd75303f",
    "67346dd692aefa25dc50dc8bcbf1ae69171e6ec7e1955c773c0e452cea36b867",
    "fbdd9a16f6eec3cab48d812f180ad4cfd0f503454196ac054acf694fd7d325d9",
    "86cde18de87c542cb55a8a6864d3ae910ef180e65e196b7f3db2e9d76eb9e45a",
    "a0f4d3776193a7e26aeeca4ec119874a97603cf8923ab0091f77ed48681f34de",
    "f076a669155a9cc7ec2bf5fb6a227fa98bcd3c5fd7fa0b99bcfc5a9e8bd7ab28",
    "9b518031ad7a868d29419cff4d3618f2dd01ee46af5897e3bce65e1af8cf682f",
    "e2aeb156c70f46d4735ea13db9238be1cb3975b74b3bd775901d67d4159f5b47",
    "ec226fdeb01e5e141edbfe8012e05ff8b0acc9b01d32a80a932cbcbdb69b3be9",
    "7a27e33146853b66f5a22315e5b7e46007506248d0054adbcd9e9caaee3fb98c",
    "7988fd08aefdba2821c8bf7c072197d9b374b19187ab590f02cee980b623db3e",
    "b1e161acb17cdcd6e91572048f9b44889e52f73a14a9faac881e1625b31caf1f",
    "8d271de70ede425ce8b883da45157c28e3289fca1451dfe522cff68de7d9788d",
    "b85997a01b601e379b5918c4a5c1305e8bf862db73df724e50cf668aa2dce490",
    "60159efd279dc6581ea13bb93c425e5d0d2c002c9b0e0bd64ce112b40fff1dc2",
    "44e255385c6c0683cb6db7610b4cdaa22ffd3ae15ed3b1fea4336e9c6e749267",
    "07bf235ea7521f9c3944737b28608184acc28a48ed2c5d6712ceb3f435da60f7",
    "8684221d51e808c54e0da0a560bcf1b15796cb479f156b85eba1dfaa6c82f434",
    "e47b231f93311f6216d55ff7095b4d28c8c0adaf884307f958b8fb6810822d8b",
    "932bb7980a2ce184442366083c0d048390f9083b52ccf3d021481dbad59f584d",
    "8530bd51d48467607436755bc87d992d24d555d25a636d90255f501c77cdb05c",
    "74edfab3c2fe2b126f538de9c03544f447fda97f5ab8bd1b5b36c254c4737cc2",
    "8cf56720a4d2b6ca5d0e28c1ac5e989ff4f7a8051c019c8eeeeda14f8e064001",
    "3d0021317a1660f456dc31358f510e909a1639f0e7214e4c394a81cef5d3a664",
    "aa7f66629d20d53dbab3f52bf23e22a28012ebca0fb1363e0ef6184bcd3dab74",
    "082ebb6f82576c0a0e2567c425e199044cd0675983f4358aa6c63b2ab9be4ac8",
    "94b6ae4dc4354c456733a73401fb36a7614eb9e4451bc9e02a38d461f76685ad",
    "185f9c07519b42189ae56b40b4e26136b40cd5e5440bee08651cb6d868c4bc7d",
    "f3b47ac6e21ef56f6e3f1bba09a6bef887c5ee1ee15fe2c339b301a17cddacc3",
    "4daab096d86951d2b0480874d332a3a777f5379fab36c2aae050a6a34bf9197f",
    "68b21c47f58306545a869758b2ad794fac2a75c0cf8155a365b87e74b6b97fcc",
    "afa24974230afd4f0f2961f8c0fb16700c3d507f8daac017377f9111386cb0d2",
    "21407ba1f0040f2237707e83a6b3c6cce3f7f911970000000000000000000000",
    "00000000000000070b10152325",
);

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn kat_seed(kat_seed_hex: &str) -> [u8; SEED_LEN] {
    let v = unhex(kat_seed_hex);
    let mut seed = [0u8; SEED_LEN];
    seed.copy_from_slice(&v);
    seed
}

fn kat_payload(i: u8) -> AttestedPayload {
    AttestedPayload {
        parent_digest: [0xC0 + i; 32],
        universe_digest: [0xD0 + i; 32],
        fuel_consumed: 0x0102_0304_0506_0708,
    }
}

#[test]
fn wire_sizes_match_fips204() {
    assert_wire_sizes_are_fips204();
}

fn assert_wire_sizes_are_fips204() {
    assert_eq!(SIGNATURE_LEN, 3309, "FIPS 204 ML-DSA-65 signature size");
    assert_eq!(PUBLIC_KEY_LEN, 1952, "FIPS 204 ML-DSA-65 public key size");
    assert_eq!(SEED_LEN, 32);
    assert_eq!(PAYLOAD_LEN, 72);
}

/// KAT 0/1: seed -> pk + deterministic signature must reproduce the frozen
/// lane-time values bit-for-bit, and 3 runs must be byte-identical.
#[test]
fn kat_frozen_vectors_reproduce_bit_for_bit_across_3_runs() {
    assert_wire_sizes_are_fips204();
    for (seed_hex, pk_hex, sig_hex, i) in [
        (KAT0_SEED_HEX, KAT0_PK_HEX, KAT0_SIG_HEX, 0u8),
        (KAT1_SEED_HEX, KAT1_PK_HEX, KAT1_SIG_HEX, 1u8),
    ] {
        let seed = kat_seed(seed_hex);
        let expected_pk = unhex(pk_hex);
        let expected_sig = unhex(sig_hex);
        let s = Signer::from_seed(seed);
        assert_eq!(
            s.public_key(),
            expected_pk,
            "public key drifted from frozen PIPELINE-KAT"
        );
        let p = kat_payload(i);
        for run in 0..3 {
            let sig = s.sign(&p);
            assert_eq!(
                sig.to_vec(),
                expected_sig,
                "run {run}: signature drifted from frozen PIPELINE-KAT"
            );
        }
    }
}

/// Verify-accept on the legitimate frozen signature (both KATs).
#[test]
fn kat_verify_accepts_legitimate() {
    for (seed_hex, pk_hex, sig_hex, i) in [
        (KAT0_SEED_HEX, KAT0_PK_HEX, KAT0_SIG_HEX, 0u8),
        (KAT1_SEED_HEX, KAT1_PK_HEX, KAT1_SIG_HEX, 1u8),
    ] {
        let _s = Signer::from_seed(kat_seed(seed_hex));
        let pk = unhex(pk_hex);
        let sig = unhex(sig_hex);
        assert!(
            Signer::verify(&pk, &kat_payload(i), &sig),
            "legitimate PIPELINE-KAT signature must verify"
        );
    }
}

/// Verify-REJECT on each of 64 sampled single-bit flips of the frozen KAT0
/// signature. Sample: stride across the full 3309-byte signature,
/// flip index = (i * SIGNATURE_LEN) / 64, bit = i % 8.
#[test]
fn kat_rejects_64_sampled_single_bit_flips() {
    let s = Signer::from_seed(kat_seed(KAT0_SEED_HEX));
    let pk = s.public_key();
    let good = s.sign(&kat_payload(0));
    assert!(Signer::verify(&pk, &kat_payload(0), &good));
    let mut flips = 0;
    for i in 0..64usize {
        let byte_idx = (i * SIGNATURE_LEN) / 64;
        let bit = i % 8;
        let mut bad = good;
        bad[byte_idx] ^= 1 << bit;
        assert!(
            !Signer::verify(&pk, &kat_payload(0), &bad),
            "corrupted signature (byte {byte_idx}, bit {bit}) must be REJECTED"
        );
        flips += 1;
    }
    assert_eq!(flips, 64);
}

/// Payload binding: the KAT0 signature is bound to its payload. Any other
/// payload must be refused: different fuel, different parent digest,
/// different universe digest, and a different payload derived by mutating
/// each payload byte position class.
#[test]
fn kat_signature_is_payload_bound() {
    let s = Signer::from_seed(kat_seed(KAT0_SEED_HEX));
    let pk = s.public_key();
    let sig_a = s.sign(&kat_payload(0));
    let p_a = kat_payload(0);
    // All divergent payloads must refuse.
    let mut variants: Vec<AttestedPayload> = Vec::new();
    // fuel change
    variants.push(AttestedPayload { fuel_consumed: 0x0102_0304_0506_0709, ..p_a });
    // parent digest change (first byte)
    let mut parent = p_a.parent_digest;
    parent[0] ^= 0x01;
    variants.push(AttestedPayload { parent_digest: parent, ..p_a });
    // universe digest change (last byte)
    let mut universe = p_a.universe_digest;
    universe[31] ^= 0x80;
    variants.push(payload_wrap(parent, universe, p_a.fuel_consumed));
    // every payload byte position, one at a time (exhaustive 72-byte flip census)
    let bytes_a = p_a.to_bytes();
    for b in 0..PAYLOAD_LEN {
        let mut m = bytes_a;
        m[b] ^= 0x01;
        let p = AttestedPayload::from_bytes(&m).expect("same length");
        variants.push(p);
    }
    assert_eq!(variants.len(), 3 + PAYLOAD_LEN);
    for (n, v) in variants.iter().enumerate() {
        assert!(
            !Signer::verify(&pk, v, &sig_a),
            "payload variant {n} must be REFUSED by the payload-A signature"
        );
    }
}

fn payload_wrap(parent: [u8; 32], universe: [u8; 32], fuel: u64) -> AttestedPayload {
    AttestedPayload { parent_digest: parent, universe_digest: universe, fuel_consumed: fuel }
}

/// Cross-key binding: a signature from KAT1's key must not verify under
/// KAT0's public key (and vice versa), for the same payload.
#[test]
fn kat_signature_is_key_bound() {
    let s0 = Signer::from_seed(kat_seed(KAT0_SEED_HEX));
    let s1 = Signer::from_seed(kat_seed(KAT1_SEED_HEX));
    let p = kat_payload(0);
    let sig0 = s0.sign(&p);
    let sig1 = s1.sign(&p);
    assert!(Signer::verify(&s0.public_key(), &p, &sig0));
    assert!(Signer::verify(&s1.public_key(), &p, &sig1));
    assert!(!Signer::verify(&s1.public_key(), &p, &sig0));
    assert!(!Signer::verify(&s0.public_key(), &p, &sig1));
}

/// Wrong-length signature / public key must fail closed.
#[test]
fn malformed_inputs_fail_closed() {
    let s = Signer::from_seed(kat_seed(KAT0_SEED_HEX));
    let pk = s.public_key();
    let sig = s.sign(&kat_payload(0));
    assert!(!Signer::verify(&pk, &kat_payload(0), &sig[..sig.len() - 1]));
    assert!(!Signer::verify(&pk, &kat_payload(0), &[],));
    assert!(!Signer::verify(&[], &kat_payload(0), &sig));
    assert!(!Signer::verify(&pk[..pk.len() - 1], &kat_payload(0), &sig));
}
