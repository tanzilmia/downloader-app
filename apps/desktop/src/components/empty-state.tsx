import { ArrowUpRight, type LucideIcon } from 'lucide-react';
import { Link } from 'react-router';
import { Button } from './ui/button';

export function EmptyState({
  icon: Icon,
  title,
  description,
  action = false,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: boolean;
}) {
  return (
    <div className="empty-state">
      <span className="empty-icon">
        <Icon size={23} strokeWidth={1.5} />
      </span>
      <h3>{title}</h3>
      <p>{description}</p>
      {action ? (
        <Button variant="outline" size="sm" asChild>
          <Link to="/">
            Add your first link <ArrowUpRight size={14} />
          </Link>
        </Button>
      ) : null}
    </div>
  );
}
