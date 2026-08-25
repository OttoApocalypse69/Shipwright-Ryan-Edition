import { DeviceList } from "./device-list";

export const metadata = { title: "Devices" };
export default function DevicesPage() { return <section className="shell section"><span className="eyebrow">Privacy-preserving identity</span><h2>Devices</h2><p className="lede">Each SRE installation uses a random UUID and local asymmetric key—not hardware fingerprinting. Revocation blocks future FTEP-authorized launches without touching local files.</p><DeviceList /></section>; }
