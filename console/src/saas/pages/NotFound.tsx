import { useLocation, useNavigate } from "react-router-dom";

const NotFound = () => {
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <div className="min-h-screen grid place-items-center bg-paper text-ink font-display">
      <div className="text-center">
        <div className="font-term text-[13px] uppercase tracking-[0.14em] text-ink/40">404</div>
        <h1 className="mt-2 text-[22px] font-bold">Page not found</h1>
        <p className="mt-1 font-term text-[12px] text-ink/55">{location.pathname}</p>
        <button
          onClick={() => navigate("/console")}
          className="mt-5 inline-flex items-center rounded-[4px] bg-biscay px-3.5 h-9 text-[13px] font-semibold text-white hover:bg-biscay-2 transition-colors"
        >
          Back to console
        </button>
      </div>
    </div>
  );
};

export default NotFound;
